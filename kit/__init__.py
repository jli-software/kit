"""KIterminal — Schlankes KI-CLI-Tool mit Multi-Provider-Support."""

__version__ = "0.10.0"


def main() -> None:
    """Entry-Point; importiert main.py erst beim Start (dort wird argv geparst)."""
    from kit.main import run
    run()
