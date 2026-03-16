from datetime import datetime, timezone
import unittest

from things_cloud.store import ThingsStore


class StoreLogbookTests(unittest.TestCase):
    def test_logbook_includes_completed_and_canceled(self) -> None:
        state = {
            "done-000000000000": {
                "e": "Task6",
                "p": {
                    "tt": "Done",
                    "tp": 0,
                    "ss": 3,
                    "st": 1,
                    "ix": 1,
                    "sp": 1773600000,
                },
            },
            "canceled-00000000": {
                "e": "Task6",
                "p": {
                    "tt": "Canceled",
                    "tp": 0,
                    "ss": 2,
                    "st": 1,
                    "ix": 2,
                    "sp": 1773600100,
                },
            },
            "open-000000000000": {
                "e": "Task6",
                "p": {"tt": "Open", "tp": 0, "ss": 0, "st": 1, "ix": 3},
            },
        }
        store = ThingsStore(state)

        tasks = store.logbook()
        titles = [t.title for t in tasks]

        self.assertIn("Done", titles)
        self.assertIn("Canceled", titles)
        self.assertNotIn("Open", titles)

    def test_logbook_respects_date_filters_for_canceled_items(self) -> None:
        stop_ts = 1773600100
        local_tz = datetime.now().astimezone().tzinfo or timezone.utc
        state = {
            "canceled-00000000": {
                "e": "Task6",
                "p": {
                    "tt": "Canceled",
                    "tp": 0,
                    "ss": 2,
                    "st": 1,
                    "ix": 2,
                    "sp": stop_ts,
                },
            }
        }
        store = ThingsStore(state)

        day = datetime.fromtimestamp(stop_ts, tz=local_tz).replace(
            hour=0, minute=0, second=0, microsecond=0
        )
        from_day = day
        to_day = day
        tasks = store.logbook(from_date=from_day, to_date=to_day)

        self.assertEqual(len(tasks), 1)
        self.assertEqual(tasks[0].title, "Canceled")


if __name__ == "__main__":
    unittest.main()
