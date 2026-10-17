#!/usr/bin/env python3
"""Measure an idle Draugr Time process on Linux.

Samples /proc for a running process and reports CPU time, wakeups (context
switches across all threads) and memory over a fixed window. Leave the
application on one screen while it runs, then repeat on another build:

    target/release/draugr-time &
    python3 scripts/measure_idle.py "$(pgrep -n draugr-time)" 60

Numbers are for comparing builds on the same machine and screen, not absolute
guarantees: other desktop activity, the compositor and the theme all add noise.
"""

import argparse
import os
import sys
import time


def cpu_seconds(pid: int) -> float:
    with open(f"/proc/{pid}/stat") as f:
        # The command name may contain spaces; fields resume after the last ')'.
        fields = f.read().rsplit(")", 1)[1].split()
    utime, stime = int(fields[11]), int(fields[12])
    return (utime + stime) / os.sysconf("SC_CLK_TCK")


def context_switches(pid: int) -> tuple[int, int]:
    voluntary = involuntary = 0
    for tid in os.listdir(f"/proc/{pid}/task"):
        try:
            with open(f"/proc/{pid}/task/{tid}/status") as f:
                for line in f:
                    if line.startswith("voluntary_ctxt_switches:"):
                        voluntary += int(line.split()[1])
                    elif line.startswith("nonvoluntary_ctxt_switches:"):
                        involuntary += int(line.split()[1])
        except FileNotFoundError:
            continue  # thread exited between listing and reading
    return voluntary, involuntary


def memory_kib(pid: int) -> dict[str, int]:
    wanted = {"Rss", "Pss", "Private_Dirty"}
    result = {}
    with open(f"/proc/{pid}/smaps_rollup") as f:
        for line in f:
            key, _, rest = line.partition(":")
            if key in wanted:
                result[key] = int(rest.split()[0])
    return result


def threads(pid: int) -> int:
    return len(os.listdir(f"/proc/{pid}/task"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("pid", type=int)
    parser.add_argument("seconds", type=float, nargs="?", default=60.0)
    args = parser.parse_args()

    if not os.path.exists(f"/proc/{args.pid}"):
        print(f"No process {args.pid}", file=sys.stderr)
        return 1

    cpu_start = cpu_seconds(args.pid)
    vol_start, invol_start = context_switches(args.pid)
    wall_start = time.monotonic()
    time.sleep(args.seconds)
    wall = time.monotonic() - wall_start
    cpu = cpu_seconds(args.pid) - cpu_start
    vol_end, invol_end = context_switches(args.pid)
    memory = memory_kib(args.pid)

    print(f"window          {wall:.1f} s")
    print(f"cpu             {cpu:.3f} s ({100 * cpu / wall:.3f}% of one core)")
    print(f"wakeups         {(vol_end - vol_start) / wall:.1f}/s voluntary, "
          f"{(invol_end - invol_start) / wall:.1f}/s involuntary")
    print(f"threads         {threads(args.pid)}")
    for key in ("Pss", "Rss", "Private_Dirty"):
        if key in memory:
            print(f"{key:<15} {memory[key] / 1024:.2f} MiB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
