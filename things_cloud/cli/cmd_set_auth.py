"""Set-auth command."""

import argparse
import getpass
import sys

from things_cloud.auth import AuthConfigError, write_auth
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
)


def cmd_set_auth(_args: argparse.Namespace) -> int:
    """Interactively configure Things Cloud credentials."""
    print("Configure Things Cloud authentication")
    email = input("Email: ").strip()
    password = getpass.getpass("Password: ")

    try:
        path = write_auth(email, password)
    except AuthConfigError as e:
        print(f"Failed to write auth config: {e}", file=sys.stderr)
        return 1

    print(colored(f"{ICONS.done} Auth saved", GREEN), colored(str(path), DIM))
    return 0


SET_AUTH_COMMAND = "set-auth"


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    subparsers.add_parser(SET_AUTH_COMMAND, help="Configure Things Cloud credentials")
    # set-auth is dispatched specially (no store/client), so we don't add it to COMMANDS
    return {}
