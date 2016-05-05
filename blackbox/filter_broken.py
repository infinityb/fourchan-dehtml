#!/usr/bin/env python
from __future__ import print_function

import sys
import json
import lib

good_ctr = 0
total_ctr = 0

for line in sys.stdin.xreadlines():
    body = json.loads(line)
    total_ctr += 1
    try:
        bout = json.loads(lib.parse_html(body))
        good_ctr += 1
    except (lib.DeHtmlError, ValueError) as e:
        print(line.strip())
        print("{} error: {}".format(type(e).__name__, e), file=sys.stderr)
        continue
    print("OK {!r}".format(bout), file=sys.stderr)

print("{} / {}".format(good_ctr, total_ctr), file=sys.stderr)
