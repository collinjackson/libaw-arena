#!/usr/bin/env python3
"""Whether a pull request to bots.json can merge itself, and the check that feeds it.

  scripts/automerge.py verdict CHECK_DIR CHANGED.json > verdict.json
      After the PR check's games: every game verified, no strikes at all, and each changed bot
      made a real move (not only ending turns) in every game it played. Exit 1 if not.
  scripts/automerge.py eligible BASE_BOTS.json HEAD_BOTS.json AUTHOR FILES.txt VERDICT.json
      The PR changes only bots.json, and only one bot in it; the file is valid JSON with no new
      fields; the author owns the bot's repo (for a removal: owns it, or added the bot); and
      the verdict is good for exactly that bot.
      Prints the reasons; exit 0 to merge.

A pull request that isn't eligible isn't rejected: it waits for a maintainer, as before."""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from build_registry import problems  # noqa: E402


def owner(repo):
    """OWNER of https://github.com/OWNER/REPO, lowercased; '' for a bot in this repo."""
    parts = (repo or "").rstrip("/").split("/")
    return parts[3].lower() if len(parts) >= 5 and parts[2] == "github.com" else ""


def verdict(check_dir, changed):
    """{ok, games, strikes, reasons} from a check's records (CHECK_DIR/MODE/game-*.json)."""
    reasons, games, strikes = [], 0, 0
    moved = {n: [] for n in changed}
    for mode in sorted(os.listdir(check_dir)):
        d = os.path.join(check_dir, mode)
        if not os.path.isdir(d):
            continue
        for f in sorted(os.listdir(d)):
            if not (f.startswith("game-") and f.endswith(".json")):
                continue
            rec = json.load(open(os.path.join(d, f)))
            games += 1
            r = subprocess.run([os.environ.get("ARENA", "./target/release/arena"), "verify", os.path.join(d, f)], capture_output=True, text=True)
            if r.returncode != 0:
                reasons.append(f"{mode}/{f}: doesn't verify: {r.stdout.strip()[-200:]}")
            n = sum(rec.get("strikes", {}).values())
            strikes += n
            if n:
                notes = sorted({m["note"] for m in rec["moves"] if m.get("note")})
                reasons.append(f"{mode}/{f}: {n} strike(s): {'; '.join(notes)[:200]}")
            seat = {b["seat"]: b["name"] for b in rec["bots"]}
            for name in changed:
                if name in seat.values():
                    s = next(k for k, v in seat.items() if v == name)
                    moved[name].append(any(m["seat"] == s and m["action"].get("type") != "end_turn" for m in rec["moves"]))
    for name, ms in moved.items():
        if not ms:
            reasons.append(f"{name}: played no games")
        elif not all(ms):
            reasons.append(f"{name}: only ended its turn in {ms.count(False)} of {len(ms)} games")
    return {"ok": not reasons and games > 0, "games": games, "strikes": strikes, "changed": sorted(changed), "reasons": reasons}


FIELDS = {"name", "repo", "commit", "path"}


def added_by(name, repo_slug):
    """GitHub login of whoever added bot `name` to bots.json: the author of the pull request
    (or, failing that, of the commit) that first added its entry. None if unknown."""
    shas = subprocess.run(["git", "log", "--format=%H", "--reverse", "-S", f'"name": "{name}"', "--", "bots.json"],
                          capture_output=True, text=True).stdout.split()
    if not shas:
        return None
    for q in (f"repos/{repo_slug}/commits/{shas[0]}/pulls", f"repos/{repo_slug}/commits/{shas[0]}"):
        r = subprocess.run(["gh", "api", q, "-q", ".[0].user.login // empty" if q.endswith("pulls") else ".author.login // empty"],
                           capture_output=True, text=True)
        if r.returncode == 0 and r.stdout.strip():
            return r.stdout.strip()
    return None


def eligible(base, head_text, author, files, v, adder=lambda name: None):
    """(ok, reasons) for auto-merging. base: the base branch's bots.json (parsed); head_text: the
    pull request's bots.json as text; adder(name): who added that bot (for removals)."""
    reasons = []
    if files != ["bots.json"]:
        reasons.append(f"changes files other than bots.json: {', '.join(f for f in files if f != 'bots.json') or 'none'}")
    try:
        head = json.loads(head_text)
        assert isinstance(head, dict) and isinstance(head.get("open"), list)
    except (ValueError, AssertionError):
        return False, reasons + ["bots.json isn't valid JSON in the expected shape ({\"open\": [...]})"]
    if {k: x for k, x in head.items() if k != "open"} != {k: x for k, x in base.items() if k != "open"}:
        reasons.append("changes bots.json outside the list of bots")
    reasons += problems(head)
    old = {b["name"]: b for b in base.get("open", [])}
    new = {b.get("name"): b for b in head["open"] if isinstance(b, dict)}
    touched = sorted(n for n in set(old) | set(new) if old.get(n) != new.get(n))
    if not touched:
        reasons.append("changes no bot")
    elif len(touched) > 1:
        reasons.append(f"changes {len(touched)} bots ({', '.join(map(str, touched))}); one bot per pull request")
    me = author.lower()
    for n in touched:
        o, e = old.get(n), new.get(n)
        if e is not None:
            extra = sorted(set(e) - FIELDS)
            if extra:
                reasons.append(f"{n}: fields other than {', '.join(sorted(FIELDS))}: {', '.join(extra)}")
            if not e.get("repo") or not e.get("commit"):
                reasons.append(f"{n}: needs a repo and a commit")
            for side, x in (("was", o), ("now", e)):
                if x is not None and owner(x.get("repo")) != me:
                    reasons.append(f"{n}: {side} in {x.get('repo') or 'this repo'}, which {author} doesn't own")
        elif not o.get("repo"):
            reasons.append(f"{n}: an example bot in this repo; removing it takes a maintainer")
        else:  # a removal: the repo's owner, or whoever added it
            who = (adder(n) or "").lower()
            if owner(o.get("repo")) != me and who != me:
                reasons.append(f"{n}: removing it takes the owner of {o.get('repo') or 'this repo'} or whoever added it"
                               + (f" ({who})" if who else ""))
    if not v.get("ok"):
        reasons.append("the check's games weren't clean: " + "; ".join(v.get("reasons", []))[:300])
    if sorted(v.get("changed", [])) != sorted(n for n in touched if n in new):
        reasons.append("the check played different bots than this change")
    return not reasons, reasons


def main(a):
    if a[:1] == ["verdict"]:
        v = verdict(a[1], json.load(open(a[2])))
        print(json.dumps(v))
        sys.exit(0 if v["ok"] else 1)
    if a[:1] == ["eligible"]:
        files = [l.strip() for l in open(a[4]) if l.strip()]
        slug = os.environ.get("GITHUB_REPOSITORY", "")
        ok, why = eligible(json.load(open(a[1])), open(a[2]).read(), a[3], files, json.load(open(a[5])),
                           adder=lambda n: added_by(n, slug) if slug else None)
        print("eligible" if ok else "needs a maintainer:\n- " + "\n- ".join(why))
        sys.exit(0 if ok else 1)
    raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
