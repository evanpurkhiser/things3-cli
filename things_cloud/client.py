"""Things Cloud API client."""

import json
import os
import time
from urllib.parse import quote
from urllib.request import Request, urlopen

BASE_URL = "https://cloud.culturedcode.com/version/1"
USER_AGENT = "ThingsMac/32209501"
CLIENT_INFO = "eyJkbSI6Ik1hYzE0LDIiLCJsciI6IlVTIiwibmYiOnRydWUsIm5rIjp0cnVlLCJubiI6IlRoaW5nc01hYyIsIm52IjoiMzIyMDk1MDEiLCJvbiI6Im1hY09TIiwib3YiOiIyNi4zLjAiLCJwbCI6ImVuLVVTIiwidWwiOiJlbi1MYXRuLVVTIn0="
APP_ID = "com.culturedcode.ThingsMac"
SCHEMA = "301"
APP_INSTANCE_ID = os.getenv("THINGS_APP_INSTANCE_ID", "things-cli")
WRITE_PUSH_PRIORITY = "10"


class ThingsCloudClient:
    def __init__(self, email: str, password: str):
        self.email = email
        self.password = password
        self.history_key: str | None = None
        self.head_index: int = 0

    def _request(self, method: str, url: str, body=None, extra_headers=None) -> dict:
        headers = {
            "Accept": "application/json",
            "Accept-Charset": "UTF-8",
            "User-Agent": USER_AGENT,
            "things-client-info": CLIENT_INFO,
            "App-Id": APP_ID,
            "Schema": SCHEMA,
            "App-Instance-Id": APP_INSTANCE_ID,
        }
        if extra_headers:
            headers.update(extra_headers)

        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json; charset=UTF-8"
            headers["Content-Encoding"] = "UTF-8"

        req = Request(url, data=data, headers=headers, method=method)
        with urlopen(req) as resp:
            content = resp.read()
            return json.loads(content) if content else {}

    def authenticate(self) -> str:
        """Fetch history-key using email/password. Returns history-key."""
        url = f"{BASE_URL}/account/{quote(self.email)}"
        result = self._request(
            "GET",
            url,
            extra_headers={"Authorization": f"Password {quote(self.password)}"},
        )
        self.history_key = result["history-key"]
        assert isinstance(self.history_key, str)
        return self.history_key

    def get_items_page(self, start_index: int) -> dict:
        """Fetch one page of history items starting at start_index."""
        assert self.history_key, "Must authenticate first"
        url = f"{BASE_URL}/history/{self.history_key}/items?start-index={start_index}"
        return self._request("GET", url)

    def get_all_items(self) -> dict[str, dict]:
        """
        Fetch and fold the full history into a current-state dict keyed by UUID.

        The sync protocol is an append-only event log. Each entry is one of:
        - t=0: full object snapshot (create/replace)
        - t=1: partial update (merge properties)
        - t=2: delete (remove object from current state)

        Replaying in order gives current state.

        Returns a flat dict: { uuid: { "e": entity_type, "p": properties } }
        for currently-existing objects only.
        """
        if not self.history_key:
            self.authenticate()

        state: dict[str, dict] = {}
        start_index = 0

        while True:
            page = self.get_items_page(start_index)
            items = page.get("items", [])
            self.head_index = page.get("current-item-index", 0)

            for item in items:
                for uuid, obj in item.items():
                    t = obj.get("t", 0)  # 0=create/full, 1=partial update
                    entity = obj.get("e")
                    props = obj.get("p", {})

                    if t == 0:
                        # Full snapshot - replace entirely
                        state[uuid] = {"e": entity, "p": dict(props)}
                    elif t == 1:
                        # Partial update - merge props
                        if uuid in state:
                            state[uuid]["p"].update(props)
                            if entity:
                                state[uuid]["e"] = entity
                        else:
                            state[uuid] = {"e": entity, "p": dict(props)}
                    elif t == 2:
                        # Delete - remove object from current state
                        state.pop(uuid, None)

            # end-total-content-size reaching latest-total-content-size means we're caught up
            end = page.get("end-total-content-size", 0)
            latest = page.get("latest-total-content-size", 0)
            if end >= latest:
                break

            start_index += len(items)

        return state

    def commit(self, changes: dict, ancestor_index: int | None = None) -> int:
        """
        Push changes to the cloud. Returns new server-head-index.

        changes: dict of { uuid: { "e": entity_type, "p": properties } }
        """
        assert self.history_key, "Must authenticate first"
        idx = ancestor_index if ancestor_index is not None else self.head_index
        url = (
            f"{BASE_URL}/history/{self.history_key}/commit?ancestor-index={idx}&_cnt=1"
        )

        # Flatten to wire format: { uuid: { "t": 1, "e": ..., "p": ... } }
        payload = {}
        for uuid, obj in changes.items():
            payload[uuid] = {"t": 1, "e": obj["e"], "p": obj["p"]}

        result = self._request(
            "POST",
            url,
            body=payload,
            extra_headers={"Push-Priority": WRITE_PUSH_PRIORITY},
        )
        new_index = result.get("server-head-index", idx)
        self.head_index = new_index
        return new_index

    def set_task_status(
        self,
        task_uuid: str,
        status: int,
        entity: str = "Task6",
        stop_date: float | None = None,
    ) -> int:
        """Set task status using observed Task6 status mutation fields."""
        return self.commit(
            {
                task_uuid: {
                    "e": entity,
                    "p": {
                        "ss": status,
                        "sp": stop_date,
                        "md": time.time(),
                    },
                }
            }
        )

    def set_task_statuses(self, updates: list[dict]) -> int:
        """Set status for multiple tasks/projects in a single cloud commit.

        updates entries must include:
          - task_uuid: str
          - status: int
          - entity: str (optional, defaults to Task6)
          - stop_date: float | None (optional)
        """
        now = time.time()
        changes: dict[str, dict] = {}
        for item in updates:
            task_uuid = item["task_uuid"]
            changes[task_uuid] = {
                "e": item.get("entity", "Task6"),
                "p": {
                    "ss": item["status"],
                    "sp": item.get("stop_date"),
                    "md": now,
                },
            }
        return self.commit(changes)

    def mark_task_done(self, task_uuid: str, entity: str = "Task6") -> int:
        """Mark a task completed using the observed Task6 mutation shape.

        Safe for non-recurring tasks and recurring instances where no coupled
        template mutation is required (e.g. fixed-schedule recurrence tp=0).
        """
        now = time.time()
        return self.set_task_status(task_uuid, status=3, entity=entity, stop_date=now)

    def mark_task_incomplete(self, task_uuid: str, entity: str = "Task6") -> int:
        """Mark task as open/incomplete (undo complete/cancel)."""
        return self.set_task_status(task_uuid, status=0, entity=entity, stop_date=None)

    def mark_task_canceled(self, task_uuid: str, entity: str = "Task6") -> int:
        """Mark task as canceled."""
        now = time.time()
        return self.set_task_status(task_uuid, status=2, entity=entity, stop_date=now)
