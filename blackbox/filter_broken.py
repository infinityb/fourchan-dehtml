#!/usr/bin/env python
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
    except lib.DeHtmlError as e:
        print("{} error: {}".format(type(e).__name__, e))
        continue
    except ValueError as e:
        print("{} error: {}".format(type(e).__name__, e))
        continue
    print("OK {!r}".format(bout))

print("{} / {}".format(good_ctr, total_ctr))
