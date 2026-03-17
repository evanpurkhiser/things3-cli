"""
Things Cloud sync protocol - wire format types and documentation.

Discovered via MITM proxy against cloud.culturedcode.com.

The sync protocol is an append-only event log hosted at:
  GET /version/1/history/{history-key}/items?start-index=N

Each page returns a list of "items". Each item is a dict of:
  { uuid: { "t": int, "e": str, "p": dict } }

  t=0: full snapshot (create or replace)
  t=1: partial update (merge p into existing object)
  t=2: delete event (object removed from current state)

Replaying in order (folding by uuid) gives current state.

Auth: GET /version/1/account/{email} with Authorization: Password {password}
      Returns { "history-key": "...", ... }
      The history-key in the URL is the only auth for subsequent requests.

Write: POST /version/1/history/{history-key}/commit?ancestor-index={N}&_cnt=1
       Body: { uuid: { "t": 1, "e": entity, "p": partial_props } }
       Returns: { "server-head-index": N }
       ancestor-index must match current server head or commit is rejected.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


# ---------------------------------------------------------------------------
# Operation type (wire field "t")
# ---------------------------------------------------------------------------

OP_CREATE = 0  # full snapshot, replaces any existing object for this uuid
OP_UPDATE = 1  # partial update, merges "p" into existing object's "p"
OP_DELETE = 2  # deletion, "p" is empty {}; creates a Tombstone if lt=True on the task


# ---------------------------------------------------------------------------
# Entity types (wire field "e")
# Versioned: Things upgrades entity schemas, keeping old versions for compat.
# Always use the highest version when creating new objects.
# ---------------------------------------------------------------------------

ENTITY_TASK = "Task6"  # current task/project/heading version
ENTITY_CHECKLIST_ITEM = "ChecklistItem3"  # current checklist item version
ENTITY_TAG = "Tag4"  # current tag version
ENTITY_AREA = "Area3"  # current area version
ENTITY_SETTINGS = "Settings5"  # app settings
ENTITY_TOMBSTONE = "Tombstone2"  # marks a deleted object
ENTITY_COMMAND = "Command"  # special one-off inbox-from-share command


# ---------------------------------------------------------------------------
# Task property keys  (wire field names inside "p")
# ---------------------------------------------------------------------------


@dataclass
class TaskProps:
    """
    Wire-format properties for Task entities (Task3/Task4/Task6).

    All timestamps are Unix epoch (float seconds). Null means unset.
    All list fields default to [] not null.
    """

    # --- Identity & content ---

    tt: str = ""
    """Title of the task/project/heading."""

    nt: Optional[str | dict] = None
    """
    Notes. Two formats depending on entity version:
    - Task3/4: XML string '<note xml:space="preserve">text</note>'
    - Task6 t=1: {"_t": "tx", "t": 1, "ch": 0, "v": "plain text"}
    - Task6 t=2: {"_t": "tx", "t": 2, "ps": [{"r": "text", "p": 0, "l": 0, "ch": hash}]}
      where ps = paragraph list, r=raw text, p=para index, l=level(indent?), ch=crc32 checksum
    """

    # --- Type / classification ---

    tp: int = 0
    """
    Item type:
      0 = to-do (leaf task)
      1 = project
      2 = heading (section divider inside a project)
    """

    # --- Status ---

    ss: int = 0
    """
    Task status:
      0 = incomplete (open)
      2 = canceled
      3 = completed
    """

    sp: Optional[float] = None
    """Stop date: Unix timestamp when task was completed or canceled. Null if open."""

    # --- Location / scheduling ---

    st: int = 0
    """
    Start location:
      0 = Inbox
      1 = Anytime
      2 = Someday
    """

    sr: Optional[int] = None
    """
    Scheduled (start) date: Unix timestamp (midnight UTC) of the date the task
    is scheduled to appear in Anytime. Maps to Things 'When' date.
    """

    tir: Optional[int] = None
    """
    Today Index Reference: Unix timestamp (midnight UTC) of the date the task
    was last moved into Today. If tir <= today's midnight, it appears in Today.
    Tasks with tir set are always in start=Anytime (st=1).
    """

    dd: Optional[int] = None
    """Deadline: Unix timestamp (midnight UTC). Null if no deadline."""

    dds: None = None
    """
    Deadline suppressed date. Always null in observed data - may be set
    when a recurring task's deadline is snoozed.
    """

    # --- Containment ---

    pr: list[str] = field(default_factory=list)
    """
    Parent project UUID(s). List with 0 or 1 element.
    Empty = task is at top level (in an area or ungrouped).
    """

    ar: list[str] = field(default_factory=list)
    """
    Area UUID(s). List with 0 or 1 element.
    A task can belong to an area directly (without a project).
    """

    agr: list[str] = field(default_factory=list)
    """
    Action group (heading) UUID(s). List with 0 or 1 element.
    Points to a heading (tp=2) task that groups this task within a project.
    Some older UUIDs are prefixed 'ACTIONGROUP-'.
    """

    # --- Tags ---

    tg: list[str] = field(default_factory=list)
    """List of Tag UUIDs applied to this task."""

    # --- Ordering ---

    ix: int = 0
    """
    Sort index within the task's list (project, area, or top-level).
    Lower = earlier in list.
    """

    ti: int = 0
    """Today sort index. Used to order tasks within the Today view."""

    do: int = 0
    """
    Due date offset. Always 0 in observed data. May be a relative offset
    in days from a base date for deadline display purposes.
    """

    # --- Recurrence ---

    rr: Optional[RecurrenceRule] = None
    """
    Recurrence rule. Null for non-repeating tasks.
    In Task3/4: stored as an opaque XML-ish string (undecoded).
    In Task6: stored as a dict - see RecurrenceRule dataclass for field details.
    Set on the template task; instances have rr=null and rt=[template_uuid].
    """

    rt: list[str] = field(default_factory=list)
    """
    Recurring template UUID(s). List with 0 or 1 element.
    On recurring instances: points to the template task that spawned this one.
    Template tasks have rr set and rt=[].
    """

    icsd: Optional[int] = None
    """
    Instance creation suppressed date: Unix timestamp.
    Set on recurring templates to pause instance creation until this date.
    """

    acrd: Optional[int] = None
    """
    After completion reference date: Unix timestamp.
    On recurring templates with tp=1 (repeat after completion), records
    the completion date of the most recent instance for scheduling the next.

    Observed completion contract from cloud history:
    - For recurring instances linked to templates with rr.tp=1, completion is
      frequently coupled with template updates in the same commit item:
      `acrd=<completion-day-midnight>` and `tir=<next-instance-day-midnight>`.
    - For templates with rr.tp=0 (fixed schedule), instance completion usually
      does not require template writes (instance `ss/sp/md` only).
    """

    # --- Checklist ---

    icc: int = 0
    """Checklist item count: total number of ChecklistItems for this task."""

    icp: bool = False
    """
    Instance creation paused: True for all projects (tp=1). Also may be
    set True to temporarily pause recurring instance generation.
    Was initially misread as "in checklist project" but is actually about
    recurring instance creation being paused.
    """

    # --- Reminders ---

    ato: Optional[int] = None
    """
    Alarm time offset: seconds since midnight of tir/sr date when the
    reminder fires. e.g. 39300 = 10:55 AM (10*3600 + 55*60).
    """

    lai: Optional[float] = None
    """
    Last alarm interaction: Unix timestamp of when the user last interacted
    with the reminder notification (dismissed, snoozed, etc.).
    """

    # --- Display / misc ---

    sb: int = 0
    """
    Evening bit: 1 = task appears in the Evening section of Today view,
    0 = normal (morning/daytime). Only meaningful when st=ANYTIME and tir is set.
    Serializes as int (not bool) on the wire.
    """

    lt: bool = False
    """
    Leaves tombstone: when True, deleting this task creates a Tombstone
    entity in the history (so other devices know to remove it). False means
    the deletion is implicit/silent. Most tasks have lt=False; it becomes
    True once the task has been synced to at least one other device.
    """

    tr: bool = False
    """Trashed: True if this task is in the Trash."""

    dl: list = field(default_factory=list)
    """Deadline list. Always [] in observed data - purpose unclear."""

    xx: Optional[dict] = None
    """
    CRDT conflict overrides. Format: {"_t": "oo", "sn": {field: version_vector}}.
    Used by the sync engine to resolve concurrent edits. Can be ignored
    when reading; do not set when writing.
    """

    # --- Timestamps ---

    cd: Optional[float] = None
    """Creation date: Unix timestamp."""

    md: Optional[float] = None
    """User modification date: Unix timestamp of last user edit."""


@dataclass
class RecurrenceRule:
    """
    Recurrence rule embedded in a task's 'rr' property.
    Set only on template tasks; instances point back via rt=[template_uuid].
    """

    tp: int = 0
    """
    Repeat type:
      0 = fixed schedule (repeats on the same day regardless of when completed)
      1 = after completion (next due date = completion date + interval)
    """

    fu: int = 256
    """
    Frequency unit bitmask:
       8 (0b000001000) = daily   -- of entries use 'dy' key
      16 (0b000010000) = monthly? -- of entries use 'dy': 0 (any day of month?)
     256 (0b100000000) = weekly  -- of entries use 'wd' key (0=Sun..6=Sat)
    """

    fa: int = 1
    """Frequency amount: every N units. e.g. fu=256, fa=2 = every 2 weeks."""

    of: list[dict] = field(default_factory=list)
    """
    Offset entries defining which days/times within the period.
    Each entry is one of:
      {"wd": N}        weekday: 0=Sunday, 1=Monday, ..., 6=Saturday
      {"dy": N}        day: -1=any/last, 0=same day each month, 1=Monday?
      {"wdo": N, "wd": M}  week-day-ordinal: Nth occurrence of weekday M in month
                           e.g. {"wdo": 3, "wd": 5} = 3rd Saturday of month
    """

    sr: Optional[int] = None
    """Start reference: Unix timestamp of the rule's start anchor date."""

    ia: Optional[int] = None
    """
    Initial anchor: Unix timestamp of when recurrence first started.
    For tp=1, updated to the latest completion date to compute next instance.
    """

    ed: int = 64092211200
    """End date: Unix timestamp. 64092211200 = year 4001, effectively "never"."""

    rc: int = 0
    """Repeat count: number of times recurred so far."""

    ts: int = 0
    """
    Task skip: -1 = ts_skip_after_first (skip first occurrence on rescheduled),
               0  = normal.
    """

    rrv: int = 4
    """Recurrence rule version. Always 4 in observed data."""


# ---------------------------------------------------------------------------
# Checklist item property keys
# ---------------------------------------------------------------------------


@dataclass
class ChecklistItemProps:
    """Wire-format properties for ChecklistItem entities."""

    tt: str = ""
    """Title / text of the checklist item."""

    ss: int = 0
    """
    Status:
      0 = incomplete
      2 = canceled
      3 = completed
    """

    sp: Optional[float] = None
    """Stop date: Unix timestamp when item was completed/canceled."""

    ts: Optional[str] = None
    """
    Parent task UUID. The checklist item belongs to this task.
    (Field name 'ts' = task, confusingly same abbrev as RecurrenceRule.ts)
    """

    ix: int = 0
    """Sort index within the parent task's checklist."""

    cd: Optional[float] = None
    """Creation date: Unix timestamp."""

    md: Optional[float] = None
    """Modification date: Unix timestamp."""

    lt: bool = False
    """Last in today (same semantics as Task.lt)."""

    xx: Optional[dict] = None
    """CRDT conflict overrides (same as Task.xx)."""


# ---------------------------------------------------------------------------
# Tag property keys
# ---------------------------------------------------------------------------


@dataclass
class TagProps:
    """Wire-format properties for Tag entities."""

    tt: str = ""
    """Tag title / name."""

    sh: Optional[str] = None
    """Keyboard shortcut (single character). Null if unset."""

    ix: int = 0
    """Sort index."""

    pn: list[str] = field(default_factory=list)
    """Parent tag UUIDs (for nested tags). Usually []. Contains at most one element."""

    xx: Optional[dict] = None
    """CRDT conflict overrides."""


# ---------------------------------------------------------------------------
# Area property keys
# ---------------------------------------------------------------------------


@dataclass
class AreaProps:
    """Wire-format properties for Area entities."""

    tt: str = ""
    """Area title / name."""

    tg: list[str] = field(default_factory=list)
    """List of Tag UUIDs applied to this area."""

    ix: int = 0
    """Sort index."""

    xx: Optional[dict] = None
    """CRDT conflict overrides."""


# ---------------------------------------------------------------------------
# Tombstone property keys
# ---------------------------------------------------------------------------


@dataclass
class TombstoneProps:
    """
    Marks a deleted object. The deleted object still exists in state
    (its entry is not removed) but the Tombstone signals it should be
    treated as deleted.
    """

    dloid: str = ""
    """Deleted object ID: UUID of the deleted entity."""

    dld: Optional[float] = None
    """Delete date: Unix timestamp when the object was deleted."""


# ---------------------------------------------------------------------------
# Command property keys
# ---------------------------------------------------------------------------


@dataclass
class CommandProps:
    """
    One-shot command entity. Used for things like 'create task from share sheet'
    that don't map cleanly to a regular entity create.
    """

    tp: int = 0
    """
    Command type:
      1 = create task from initial fields (e.g. from iOS share sheet)
    """

    cd: Optional[int] = None
    """Creation date: Unix timestamp."""

    if_: Optional[dict] = None
    """
    Initial fields for tp=1: {"id": uuid, "tt": title, "nt": notes}
    The 'id' is the UUID of the Task that should be created.
    Wire field name is 'if' (reserved in Python, hence if_).
    """


# ---------------------------------------------------------------------------
# Status / start / type constants
# ---------------------------------------------------------------------------


class TaskStatus:
    INCOMPLETE = 0
    CANCELED = 2
    COMPLETED = 3


class TaskStart:
    INBOX = 0
    ANYTIME = 1
    SOMEDAY = 2


class TaskType:
    TODO = 0
    PROJECT = 1
    HEADING = 2


class ChecklistStatus:
    INCOMPLETE = 0
    CANCELED = 2
    COMPLETED = 3


class RecurrenceType:
    FIXED_SCHEDULE = 0
    AFTER_COMPLETION = 1


class FrequencyUnit:
    DAILY = 8
    MONTHLY = 16  # tentative - observed with dy:0 offsets
    WEEKLY = 256
