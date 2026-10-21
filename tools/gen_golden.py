#!/usr/bin/env python3
"""Generate the golden biquad coefficients from the author's Python library.

The Rust test `golden_biquads` (test 14 of the test matrix) asserts that our
RBJ peaking EQ coefficients match an *independent* implementation of the same
DSP. The reference is the author's own Python library in their vault,
`3_resources/programming_python/audio_filters/butterworth_filter.py`, whose
`make_peak` factory follows the RBJ Audio EQ Cookbook.

This script is a development tool. It is not part of the build, and the file it
writes is committed so that the test can run without Python.

Usage:

    python3 tools/gen_golden.py [--library DIR] [--out FILE] [--check]

    --library DIR   the directory that holds the `audio_filters` package
                    (default: ../../3_resources/programming_python, relative to
                    the repository root)
    --out FILE      where to write the golden file
                    (default: crates/sehrnoise-core/tests/data/golden_biquads.json)
    --check         do not write anything; fail if the file on disk differs

The coefficients are written normalised by `a0`, which is the convention the
Rust code uses (`biquad::Coeffs` always has `a0 == 1`).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LIBRARY = REPO_ROOT.parent.parent / "3_resources" / "programming_python"
DEFAULT_OUT = REPO_ROOT / "crates" / "sehrnoise-core" / "tests" / "data" / "golden_biquads.json"

# The sample rate every case is designed at.
SAMPLE_RATE = 48_000

# The band centres of the bank, in Hz, as whole numbers.
#
# The Python factory takes an integer frequency, so the centres are rounded
# here and the *same* rounded values are used by the Rust test. What is being
# compared is the coefficient arithmetic, not the band table.
BAND_CENTRES_HZ = [31, 73, 173, 411, 974, 2310, 5476, 12986]

# The Q of the band bank: `1 / (r - 1/r)` with `r = 2.371373`.
BAND_Q = 1.0 / (2.371373 - 1.0 / 2.371373)

# The gains every band centre is designed at.
GAINS_DB = [-12.0, 0.0, 12.0]


def load_make_peak(library_dir: Path):
    """Import `make_peak` from the author's Python library."""
    if not (library_dir / "audio_filters" / "butterworth_filter.py").is_file():
        raise SystemExit(
            f"cannot find the audio_filters package in {library_dir}\n"
            "pass --library with the directory that holds it"
        )
    sys.path.insert(0, str(library_dir))
    from audio_filters.butterworth_filter import make_peak  # noqa: PLC0415

    return make_peak


def cases() -> list[dict]:
    """The list of cases the golden file holds."""
    out: list[dict] = []
    for frequency in BAND_CENTRES_HZ:
        for gain_db in GAINS_DB:
            out.append(
                {
                    "frequency_hz": frequency,
                    "q": BAND_Q,
                    "gain_db": gain_db,
                }
            )

    # Two extra cases that are not band centres, so the test also covers a
    # textbook Q and a frequency well away from the band table.
    out.append({"frequency_hz": 1000, "q": 1.0 / math.sqrt(2.0), "gain_db": 6.0})
    out.append({"frequency_hz": 440, "q": 0.7071067811865476, "gain_db": -6.0})

    return out


def coefficients(make_peak, case: dict) -> dict:
    """The coefficients of one case, normalised by `a0`."""
    filt = make_peak(
        frequency=int(case["frequency_hz"]),
        samplerate=SAMPLE_RATE,
        gain_db=case["gain_db"],
        q_factor=case["q"],
    )

    a0, a1, a2 = filt.a_coeffs
    b0, b1, b2 = filt.b_coeffs

    return {
        "b0": b0 / a0,
        "b1": b1 / a0,
        "b2": b2 / a0,
        "a1": a1 / a0,
        "a2": a2 / a0,
    }


def build(library_dir: Path) -> dict:
    make_peak = load_make_peak(library_dir)
    built = []
    for case in cases():
        entry = dict(case)
        entry.update(coefficients(make_peak, case))
        built.append(entry)

    return {
        "generator": "tools/gen_golden.py",
        "source": "audio_filters.butterworth_filter.make_peak",
        "sample_rate": SAMPLE_RATE,
        "note": "coefficients are normalised by a0",
        "cases": built,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, default=DEFAULT_LIBRARY)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the file on disk is not what this script would write",
    )
    args = parser.parse_args()

    document = build(args.library)
    text = json.dumps(document, indent=2) + "\n"

    if args.check:
        if not args.out.is_file():
            print(f"{args.out} does not exist")
            return 1
        if args.out.read_text() != text:
            print(f"{args.out} is out of date; run tools/gen_golden.py")
            return 1
        print(f"{args.out} is up to date")
        return 0

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(text)
    print(f"wrote {len(document['cases'])} cases to {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
