"""
In-memory store that builds current state from Things Cloud history.
"""

import time
from bisect import bisect_left, bisect_right
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Optional

from things_cloud.schema import (
    ENTITY_AREA,
    ENTITY_CHECKLIST_ITEM,
    ENTITY_TAG,
    ENTITY_TASK,
    ChecklistStatus,
    RecurrenceType,
    TaskStart,
    TaskStatus,
    TaskType,
)

START_ANYTIME = TaskStart.ANYTIME
START_INBOX = TaskStart.INBOX
START_SOMEDAY = TaskStart.SOMEDAY
STATUS_INCOMPLETE = TaskStatus.INCOMPLETE
STATUS_CANCELED = TaskStatus.CANCELED
STATUS_COMPLETED = TaskStatus.COMPLETED
TYPE_TODO = TaskType.TODO
TYPE_PROJECT = TaskType.PROJECT
TYPE_HEADING = TaskType.HEADING
RECURRENCE_FIXED_SCHEDULE = RecurrenceType.FIXED_SCHEDULE
RECURRENCE_AFTER_COMPLETION = RecurrenceType.AFTER_COMPLETION
LOCAL_TZ = datetime.now().astimezone().tzinfo or timezone.utc


def _lcp_len(a: str, b: str) -> int:
    limit = min(len(a), len(b))
    i = 0
    while i < limit and a[i] == b[i]:
        i += 1
    return i


def _shortest_unique_prefixes(ids: list[str]) -> dict[str, str]:
    if not ids:
        return {}

    ordered = sorted(ids)
    result: dict[str, str] = {}

    for i, value in enumerate(ordered):
        left = _lcp_len(value, ordered[i - 1]) if i > 0 else 0
        right = _lcp_len(value, ordered[i + 1]) if i + 1 < len(ordered) else 0
        need = max(left, right) + 1
        result[value] = value[:need]

    return result


def _ts_to_dt(ts) -> Optional[datetime]:
    if ts is None:
        return None
    return datetime.fromtimestamp(ts, tz=timezone.utc)


@dataclass
class Tag:
    uuid: str
    title: str
    shortcut: Optional[str] = None
    index: int = 0
    parent_uuid: Optional[str] = None


@dataclass
class Area:
    uuid: str
    title: str
    tags: list[str] = field(default_factory=list)
    index: int = 0


@dataclass
class Task:
    uuid: str
    title: str
    status: int = STATUS_INCOMPLETE
    start: int = START_INBOX
    type: int = TYPE_TODO
    entity: str = "Task6"  # wire entity type: Task3, Task4, Task6
    notes: Optional[str] = None
    project: Optional[str] = None  # UUID
    area: Optional[str] = None  # UUID
    action_group: Optional[str] = None  # agr: heading/group UUID
    tags: list[str] = field(default_factory=list)  # UUIDs
    trashed: bool = False
    deadline: Optional[datetime] = None
    start_date: Optional[datetime] = None
    stop_date: Optional[datetime] = None
    creation_date: Optional[datetime] = None
    modification_date: Optional[datetime] = None
    index: int = 0
    today_index: int = 0
    today_index_reference: Optional[int] = None  # raw unix ts from tir (tir)
    leaves_tombstone: bool = False  # lt: True once synced to another device
    instance_creation_paused: bool = False  # icp: True for all projects
    evening: bool = False  # sb: True = appears in Evening section of Today
    recurrence_rule: Optional[dict] = None  # rr: recurrence template rule
    recurrence_templates: list[str] = field(default_factory=list)  # rt: template refs
    checklist_items: list["ChecklistItem"] = field(default_factory=list)

    @property
    def is_incomplete(self) -> bool:
        return self.status == STATUS_INCOMPLETE

    @property
    def is_completed(self) -> bool:
        return self.status == STATUS_COMPLETED

    @property
    def is_canceled(self) -> bool:
        return self.status == STATUS_CANCELED

    @property
    def is_todo(self) -> bool:
        return self.type == TYPE_TODO

    @property
    def is_project(self) -> bool:
        return self.type == TYPE_PROJECT

    @property
    def is_heading(self) -> bool:
        return self.type == TYPE_HEADING

    @property
    def is_today(self) -> bool:
        """Task appears in Things Today view.

        A task is in Today when:
          - st == ANYTIME (start=1)
          - sr is set and sr <= today's local midnight

        The local date is used because Things compares scheduled dates in the
        device's local timezone. Tasks with a scheduled date in the past that
        were not completed remain in Today (they roll over day-to-day).

        The `sb` (evening) bit controls whether the task appears in the regular
        section (sb=0) or the "This Evening" section (sb=1) of Today.

        Note: this matches the `things.py` SQLite library's Today prediction
        logic: `startDate IS NOT NULL AND start=Anytime AND startDate <= today`.
        """
        if self.start_date is None:
            return False
        if self.start != 1:  # must be Anytime
            return False
        # sr is stored as UTC midnight of the scheduled date. Compare dates in UTC
        # (Things stores dates as day-granularity values, timezone-independent).
        today_utc = datetime.now(tz=timezone.utc).replace(
            hour=0, minute=0, second=0, microsecond=0
        )
        return self.start_date <= today_utc

    @property
    def is_inbox(self) -> bool:
        return self.start == START_INBOX and not self.project and not self.area

    @property
    def in_someday(self) -> bool:
        # Tasks marked Someday can also carry a concrete future start date.
        # In Things UI those behave like scheduled/upcoming items, not plain
        # Someday backlog items.
        return self.start == START_SOMEDAY and self.start_date is None

    @property
    def is_recurring(self) -> bool:
        # rr exists on templates, rt points to template on generated instances.
        return bool(self.recurrence_rule) or bool(self.recurrence_templates)

    @property
    def is_recurrence_template(self) -> bool:
        return bool(self.recurrence_rule) and not self.recurrence_templates

    @property
    def is_recurrence_instance(self) -> bool:
        return bool(self.recurrence_templates) and not self.recurrence_rule


@dataclass
class ChecklistItem:
    uuid: str
    title: str
    task_uuid: str  # parent task UUID
    status: int = ChecklistStatus.INCOMPLETE
    index: int = 0

    @property
    def is_incomplete(self) -> bool:
        return self.status == ChecklistStatus.INCOMPLETE

    @property
    def is_completed(self) -> bool:
        return self.status == ChecklistStatus.COMPLETED

    @property
    def is_canceled(self) -> bool:
        return self.status == ChecklistStatus.CANCELED


@dataclass(frozen=True)
class ProjectProgress:
    total: int = 0
    done: int = 0


class ThingsStore:
    """
    Builds and queries a current-state snapshot from raw history items.
    """

    def __init__(self, raw_state: dict[str, dict]):
        self._tasks: dict[str, Task] = {}
        self._areas: dict[str, Area] = {}
        self._tags: dict[str, Tag] = {}
        self._tag_by_title: dict[str, str] = {}  # title -> uuid
        self._project_progress: dict[str, ProjectProgress] = {}
        self._short_ids: dict[str, str] = {}
        self._markable_ids: set[str] = set()
        self._markable_ids_sorted: list[str] = []

        self._area_ids_sorted: list[str] = []
        self._task_ids_sorted: list[str] = []

        self._build(raw_state)
        self._build_project_progress_index()
        self._short_ids = _shortest_unique_prefixes(self._short_id_domain(raw_state))
        self._build_mark_indexes()
        self._area_ids_sorted = sorted(self._areas.keys())
        self._task_ids_sorted = sorted(self._tasks.keys())

    def _build_project_progress_index(self) -> None:
        totals: dict[str, int] = {}
        dones: dict[str, int] = {}

        for task in self._tasks.values():
            if task.trashed or not task.is_todo:
                continue

            project_uuid = self.effective_project_uuid(task)
            if not project_uuid:
                continue

            totals[project_uuid] = totals.get(project_uuid, 0) + 1
            if task.is_completed:
                dones[project_uuid] = dones.get(project_uuid, 0) + 1

        self._project_progress = {
            project_uuid: ProjectProgress(total=total, done=dones.get(project_uuid, 0))
            for project_uuid, total in totals.items()
        }

    def _build_mark_indexes(self) -> None:
        markable = [
            task
            for task in self._tasks.values()
            if not task.trashed and not task.is_heading and task.entity == ENTITY_TASK
        ]

        self._markable_ids = {task.uuid for task in markable}
        self._markable_ids_sorted = sorted(self._markable_ids)

    def _short_id_domain(self, raw_state: dict[str, dict]) -> list[str]:
        ids: list[str] = []
        for uuid, obj in raw_state.items():
            entity = ""
            if isinstance(obj, dict):
                entity = obj.get("e", "")

            if entity in {"Tombstone", "Tombstone2"}:
                continue
            if isinstance(uuid, str) and uuid.startswith("TOMBSTONE-"):
                continue

            ids.append(uuid)
        return ids

    def _build(self, raw_state: dict[str, dict]):
        checklist_items: list[ChecklistItem] = []

        for uuid, obj in raw_state.items():
            entity = obj.get("e", "")
            p = obj.get("p", {})

            if entity.startswith("Task"):
                self._tasks[uuid] = self._parse_task(uuid, p, entity)
            elif entity.startswith("Area"):
                self._areas[uuid] = self._parse_area(uuid, p)
            elif entity.startswith("Tag"):
                tag = self._parse_tag(uuid, p)
                self._tags[uuid] = tag
                if tag.title:
                    self._tag_by_title[tag.title] = uuid
            elif entity == ENTITY_CHECKLIST_ITEM:
                checklist_items.append(self._parse_checklist_item(uuid, p))

        # Attach checklist items to their parent tasks, sorted by index
        by_task: dict[str, list[ChecklistItem]] = {}
        for item in checklist_items:
            if item.task_uuid in self._tasks:
                by_task.setdefault(item.task_uuid, []).append(item)

        for task_uuid, items in by_task.items():
            items.sort(key=lambda i: i.index)
            self._tasks[task_uuid].checklist_items = items

    def _parse_task(self, uuid: str, p: dict, entity: str = "Task6") -> Task:
        notes = p.get("nt")
        if isinstance(notes, dict):
            t = notes.get("t")
            if t == 1:
                notes = notes.get("v") or None
            elif t == 2:
                paragraphs = notes.get("ps") or []
                notes = "\n".join(para.get("r", "") for para in paragraphs) or None
            else:
                notes = None
        if isinstance(notes, str):
            notes = notes.strip() or None

        # project and area are lists in the wire format
        project_list = p.get("pr") or []
        area_list = p.get("ar") or []
        action_group_list = p.get("agr") or []

        return Task(
            uuid=uuid,
            title=p.get("tt") or "",
            status=p.get("ss", STATUS_INCOMPLETE),
            start=p.get("st", START_INBOX),
            type=p.get("tp", TYPE_TODO),
            entity=entity,
            notes=notes or None,
            project=project_list[0] if project_list else None,
            area=area_list[0] if area_list else None,
            action_group=action_group_list[0] if action_group_list else None,
            tags=p.get("tg") or [],
            trashed=bool(p.get("tr", False)),
            deadline=_ts_to_dt(p.get("dd")),
            start_date=_ts_to_dt(p.get("sr")),
            stop_date=_ts_to_dt(p.get("sp")),
            creation_date=_ts_to_dt(p.get("cd")),
            modification_date=_ts_to_dt(p.get("md")),
            index=p.get("ix", 0),
            today_index=p.get("ti", 0),
            today_index_reference=p.get("tir") or None,
            leaves_tombstone=bool(p.get("lt", False)),
            instance_creation_paused=bool(p.get("icp", False)),
            evening=bool(p.get("sb", 0)),
            recurrence_rule=p.get("rr"),
            recurrence_templates=p.get("rt") or [],
        )

    def _parse_checklist_item(self, uuid: str, p: dict) -> "ChecklistItem":
        ts = p.get("ts") or []
        task_uuid = ts[0] if ts else ""
        return ChecklistItem(
            uuid=uuid,
            title=p.get("tt") or "",
            task_uuid=task_uuid,
            status=p.get("ss", ChecklistStatus.INCOMPLETE),
            index=p.get("ix", 0),
        )

    def _parse_area(self, uuid: str, p: dict) -> Area:
        return Area(
            uuid=uuid,
            title=p.get("tt") or "",
            tags=p.get("tg") or [],
            index=p.get("ix", 0),
        )

    def _parse_tag(self, uuid: str, p: dict) -> Tag:
        pn = p.get("pn") or []
        return Tag(
            uuid=uuid,
            title=p.get("tt") or "",
            shortcut=p.get("sh"),
            index=p.get("ix", 0),
            parent_uuid=pn[0] if pn else None,
        )

    # --- Query API ---

    def tasks(
        self,
        status: Optional[int] = STATUS_INCOMPLETE,
        trashed: bool = False,
        type: Optional[int] = None,
    ) -> list[Task]:
        results = []
        for task in self._tasks.values():
            if trashed is not None and task.trashed != trashed:
                continue
            if status is not None and task.status != status:
                continue
            if type is not None and task.type != type:
                continue
            if task.is_heading:
                continue
            results.append(task)
        return sorted(results, key=lambda t: t.index)

    def today(self) -> list[Task]:
        """Tasks in Today view, ordered like the app's Today list."""
        results = [
            t
            for t in self._tasks.values()
            if not t.trashed
            and t.status == STATUS_INCOMPLETE
            and not t.is_heading
            and not t.is_project
            and t.title.strip()
            and t.entity == ENTITY_TASK  # Task6 only; skip legacy Task3/Task4
            and t.is_today
        ]

        def _today_sort_key(task: Task):
            # Things uses `ti` (today index) where larger values are higher in the
            # list. `ti=0` behaves like an "unset" value and currently appears
            # before indexed items in Today.
            if task.today_index == 0:
                sr_ts = int(task.start_date.timestamp()) if task.start_date else 0
                return (0, -sr_ts, -task.index)
            return (1, -task.today_index, -task.index)

        return sorted(results, key=_today_sort_key)

    def inbox(self) -> list[Task]:
        """Tasks in Things Inbox view.

        Only Task6 entities are returned — legacy Task3/Task4 items predate
        the current sync engine and are not shown by the Things app.
        """
        results = [
            t
            for t in self._tasks.values()
            if not t.trashed
            and t.status == STATUS_INCOMPLETE
            and t.start == START_INBOX
            and self.effective_project_uuid(t) is None
            and self.effective_area_uuid(t) is None
            and not t.is_project
            and not t.is_heading
            and t.title.strip()
            and t.creation_date is not None  # skip partial/ghost items lacking cd
            and t.entity == ENTITY_TASK  # Task6 only; skip legacy Task3/Task4
        ]
        return sorted(results, key=lambda t: t.index)

    def anytime(self) -> list[Task]:
        """Tasks in Things Anytime view.

        Includes open Task6 to-dos with st=Anytime where the scheduled date
        is unset or not in the future. This includes tasks that are in Today.
        """
        today_utc = datetime.now(tz=timezone.utc).replace(
            hour=0, minute=0, second=0, microsecond=0
        )

        def _project_visible(task: Task) -> bool:
            project_uuid = self.effective_project_uuid(task)
            if not project_uuid:
                return True
            project = self._tasks.get(project_uuid)
            if not project:
                return True
            if project.trashed or project.status != STATUS_INCOMPLETE:
                return False
            if project.start == START_SOMEDAY:
                return False
            if project.start_date and project.start_date > today_utc:
                return False
            return True

        results = [
            t
            for t in self._tasks.values()
            if not t.trashed
            and t.status == STATUS_INCOMPLETE
            and t.start == START_ANYTIME
            and not t.is_project
            and not t.is_heading
            and t.title.strip()
            and t.entity == ENTITY_TASK
            and (t.start_date is None or t.start_date <= today_utc)
            and _project_visible(t)
        ]
        return sorted(results, key=lambda t: t.index)

    def someday(self) -> list[Task]:
        """Tasks in Things Someday view."""
        results = [
            t
            for t in self._tasks.values()
            if not t.trashed
            and t.status == STATUS_INCOMPLETE
            and t.start == START_SOMEDAY
            and not t.is_heading
            and t.title.strip()
            and t.entity == ENTITY_TASK
            and not t.is_recurrence_template
            and t.start_date is None
            and (t.is_project or self.effective_project_uuid(t) is None)
        ]
        return sorted(results, key=lambda t: t.index)

    def logbook(
        self, from_date: Optional[datetime] = None, to_date: Optional[datetime] = None
    ) -> list[Task]:
        """Completed and canceled tasks, filtered by stop-date range."""
        results: list[Task] = []
        for task in self._tasks.values():
            if task.trashed or task.status not in {STATUS_COMPLETED, STATUS_CANCELED}:
                continue
            if task.is_heading:
                continue
            if task.entity != ENTITY_TASK:
                continue
            if task.stop_date is None:
                continue

            stop_day = task.stop_date.astimezone(LOCAL_TZ).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            if from_date and stop_day < from_date:
                continue
            if to_date and stop_day > to_date:
                continue
            results.append(task)

        return sorted(
            results,
            key=lambda t: (
                -(t.stop_date.timestamp() if t.stop_date else 0),
                -t.index,
            ),
        )

    def effective_project_uuid(self, task: Task) -> Optional[str]:
        """Resolve effective project, including heading-based containment."""
        if task.project:
            return task.project
        if task.action_group:
            heading = self._tasks.get(task.action_group)
            if heading and heading.project:
                return heading.project
        return None

    def effective_area_uuid(self, task: Task) -> Optional[str]:
        """Resolve effective area through task/project/heading relationships."""
        if task.area:
            return task.area

        project_uuid = self.effective_project_uuid(task)
        if project_uuid:
            project = self._tasks.get(project_uuid)
            if project and project.area:
                return project.area

        if task.action_group:
            heading = self._tasks.get(task.action_group)
            if heading and heading.area:
                return heading.area

        return None

    def projects(self, status: Optional[int] = STATUS_INCOMPLETE) -> list[Task]:
        results = [
            t
            for t in self._tasks.values()
            if not t.trashed
            and t.is_project
            and t.entity == ENTITY_TASK  # Task6 only; skip legacy Task3/Task4
            and (status is None or t.status == status)
        ]
        return sorted(results, key=lambda t: t.index)

    def areas(self) -> list[Area]:
        return sorted(self._areas.values(), key=lambda a: a.index)

    def tags(self) -> list[Tag]:
        return sorted(
            [t for t in self._tags.values() if t.title and t.title.strip()],
            key=lambda t: t.index,
        )

    def get_task(self, uuid: str) -> Optional[Task]:
        return self._tasks.get(uuid)

    def get_area(self, uuid: str) -> Optional[Area]:
        return self._areas.get(uuid)

    def get_tag(self, uuid: str) -> Optional[Tag]:
        return self._tags.get(uuid)

    def resolve_tag_title(self, uuid: str) -> str:
        tag = self._tags.get(uuid)
        if tag and tag.title and tag.title.strip():
            return tag.title
        return uuid

    def resolve_area_title(self, uuid: str) -> str:
        area = self._areas.get(uuid)
        return area.title if area else uuid

    def resolve_project_title(self, uuid: str) -> str:
        task = self._tasks.get(uuid)
        if task and task.title.strip():
            return task.title
        if not uuid:
            return "(project)"
        return f"(project {uuid[:8]})"

    def short_id(self, uuid: str) -> str:
        return self._short_ids.get(uuid, uuid)

    def project_progress(self, project_uuid: str) -> ProjectProgress:
        return self._project_progress.get(project_uuid, ProjectProgress())

    def unique_prefix_length(self, ids: list[str]) -> int:
        """Return the column width needed so every displayed prefix is globally unique.

        Uses the precomputed globally-unique short IDs (unique across all
        non-tombstone entities) so that any prefix shown in a view will
        resolve unambiguously when passed to ``mark``.
        """
        if not ids:
            return 0

        max_need = 1
        for uid in ids:
            short = self._short_ids.get(uid)
            if short is not None:
                max_need = max(max_need, len(short))
            else:
                # ID not in the global domain (shouldn't happen); fall back
                # to showing enough to be reasonably identifiable.
                max_need = max(max_need, 6)
        return max_need

    @staticmethod
    def _resolve_prefix(
        identifier: str,
        items: dict[str, object],
        sorted_ids: list[str],
        label: str = "Item",
    ) -> tuple[object | None, str, list]:
        """Generic prefix resolver for any UUID-keyed dict.

        Returns ``(item, error_message, ambiguous_matches)``.
        """
        ident = identifier.strip()
        if not ident:
            return None, f"Missing {label.lower()} identifier.", []

        exact = items.get(ident)
        if exact:
            return exact, "", []

        start = bisect_left(sorted_ids, ident)
        end = bisect_right(sorted_ids, ident + "\uffff")

        match_count = end - start
        if match_count == 1:
            item = items.get(sorted_ids[start])
            if item:
                return item, "", []

        if match_count > 1:
            matches = [items[sorted_ids[i]] for i in range(start, min(end, start + 10))]
            remaining = match_count - len(matches)
            msg = f"Ambiguous {label.lower()} id prefix."
            if remaining:
                msg += f" ({match_count} matches, showing first {len(matches)})"
            return None, msg, matches

        return None, f"{label} not found: {identifier}", []

    def resolve_mark_identifier(
        self, identifier: str
    ) -> tuple[Optional[Task], str, list[Task]]:
        """Resolve *identifier* (full UUID or short-id prefix) to a markable Task.

        Returns ``(task, error_message, ambiguous_matches)``.
        On success *task* is set and the other two are empty/blank.
        On ambiguity *task* is ``None``, *error_message* describes the problem,
        and *ambiguous_matches* contains the conflicting tasks (up to 10).
        """
        # _markable_ids is a subset of _tasks, so build a filtered view
        markable = {uid: self._tasks[uid] for uid in self._markable_ids}
        return self._resolve_prefix(
            identifier, markable, self._markable_ids_sorted, label="Item"
        )

    def resolve_area_identifier(
        self, identifier: str
    ) -> tuple[Optional[Area], str, list[Area]]:
        """Resolve *identifier* (full UUID or short-id prefix) to an Area.

        Returns ``(area, error_message, ambiguous_matches)``.
        """
        return self._resolve_prefix(
            identifier, self._areas, self._area_ids_sorted, label="Area"
        )

    def resolve_task_identifier(
        self, identifier: str
    ) -> tuple[Optional[Task], str, list[Task]]:
        """Resolve *identifier* to any task/project/heading."""
        return self._resolve_prefix(
            identifier, self._tasks, self._task_ids_sorted, label="Task"
        )
