"""GoogleSQL's own parser verdict per statement: execute_query --mode=parse, MAXIMUM features.

Usage: oracle.py RESULTS_JSONL CACHE_JSON EXECUTE_QUERY_BINARY. Verdicts are cached by text.
"""
import concurrent.futures, json, subprocess, sys
rows = [json.loads(l) for l in open(sys.argv[1])]
cache = {}
try:
    cache = json.load(open(sys.argv[2]))
except FileNotFoundError:
    pass
def verdict(sql):
    out = subprocess.run([sys.argv[3], "--mode=parse", "--sql_mode=script", "--enabled_language_features=MAXIMUM", "-"],
                         input=sql, capture_output=True, text=True, timeout=60).stdout
    return "accept" if out.startswith("Script [") else ("reject: " + out.splitlines()[0][:160] if out else "reject: (no output)")
todo = sorted({r["sql"] for r in rows if "sql" in r and r["sql"] not in cache})
with concurrent.futures.ThreadPoolExecutor(3) as pool:
    for sql, v in zip(todo, pool.map(verdict, todo)):
        cache[sql] = v
json.dump(cache, open(sys.argv[2], "w"))
print(len(cache), "verdicts")
