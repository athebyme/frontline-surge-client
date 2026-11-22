"""Check shared game data against a checkout of the matching dedicated server."""
import argparse
import sys
from pathlib import Path

CONTRACT_FILES = (
    "src/wire.rs",
    "src/world_wire.rs",
    "src/arsenal.rs",
    "src/fracture.rs",
    "assets/level.json",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server", type=Path, help="path to frontline-surge-server")
    args = parser.parse_args()
    client = Path(__file__).resolve().parents[1]
    server = args.server.resolve()
    if server == client or not (server / "src/server.rs").is_file():
        parser.error("expected a separate checkout of frontline-surge-server")

    differences = []
    for name in CONTRACT_FILES:
        try:
            client_text = (client / name).read_text(encoding="utf-8")
            server_text = (server / name).read_text(encoding="utf-8")
        except OSError as error:
            differences.append(str(error))
            continue
        if client_text != server_text:
            differences.append(f"{name} differs")

    if differences:
        print("\n".join(differences), file=sys.stderr)
        return 1
    print("Both checkouts have matching wire format, map, and shared gameplay rules.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
