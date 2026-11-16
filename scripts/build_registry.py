#!/usr/bin/env python3
"""Gather every open bot in bots.json into bots/out/NAME.wasm, and write
bots/out/manifest.json: [{name, wasm, repo, commit, sha256}], which `arena round-robin
--manifest` reads.

A submitted bot is {"name", "repo", "commit", "wasm", "sha256"}: its owner builds the module and
publishes it in their own repo, and the arena downloads it and refuses it unless its sha256
matches. Nothing of theirs is built or run here outside the sandbox. repo is
https://github.com/OWNER/REPO, commit the full 40-hex commit of the source it was built from (for
readers; not built), and wasm a URL in that same repo: a release asset
(https://github.com/OWNER/REPO/releases/download/TAG/FILE.wasm) or a file at a commit
(https://raw.githubusercontent.com/OWNER/REPO/COMMIT/PATH.wasm).

An example bot in this repo is {"name", "repo": "", "commit": "", "path"} and is built here from
path, at this repo's own commit.

Modules are kept in bots/cache/ (downloads by sha256; example builds by the last commit that
touched the bot or the SDK, and the Rust toolchain), which the workflows keep between runs.

Usage: scripts/build_registry.py [--only NAME,...] [--changed-since REF] [--check]
  --check validates bots.json without building.
  --changed-since REF builds the bots whose entries differ from bots.json at git REF, plus this
  repo's own example bots to play them against (a pull request's check).
"""
import hashlib, json, os, re, shutil, subprocess, sys, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NAME = re.compile(r"^[a-z0-9][a-z0-9_-]{0,31}$")
REPO = re.compile(r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
SHA = re.compile(r"^[0-9a-f]{40}$")
PATH = re.compile(r"^(\.|[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)*)$")
HASH = re.compile(r"^[0-9a-f]{64}$")
SEG = r"[A-Za-z0-9_.+-]+"
WASM = re.compile(r"^https://(?:github\.com/(?P<o1>[A-Za-z0-9_.-]+)/(?P<r1>[A-Za-z0-9_.-]+)/releases/download/" + SEG + "/" + SEG
                  + r"|raw\.githubusercontent\.com/(?P<o2>[A-Za-z0-9_.-]+)/(?P<r2>[A-Za-z0-9_.-]+)/[0-9a-f]{40}(?:/" + SEG + ")+)$")
MAX_WASM = 32 << 20
EXAMPLE = {"name", "repo", "commit", "path"}
SUBMITTED = {"name", "repo", "commit", "wasm", "sha256"}


def wasm_repo(url):
    """https://github.com/OWNER/REPO for a wasm URL the registry accepts (lowercased), else None."""
    m = WASM.match(url or "")
    if not m or ".." in url:
        return None
    o, r = (m["o1"], m["r1"]) if m["o1"] else (m["o2"], m["r2"])
    return f"https://github.com/{o}/{r}".lower()


def problems(reg):
    out, seen = [], set()
    for i, b in enumerate(reg.get("open", [])):
        n = b.get("name", "")
        where = f"open[{i}] {n!r}"
        if not NAME.match(n):
            out.append(f"{where}: name must be 1-32 of a-z 0-9 _ -")
        if n in seen:
            out.append(f"{where}: name used twice")
        seen.add(n)
        repo, commit = b.get("repo", ""), b.get("commit", "")
        if not repo and not commit:  # an example bot in this repo
            extra = sorted(set(b) - EXAMPLE)
            if extra:
                out.append(f"{where}: an example bot has only {', '.join(sorted(EXAMPLE))}, not {', '.join(extra)}")
            path = b.get("path", ".")
            if not isinstance(path, str) or not PATH.match(path) or ".." in path.split("/"):
                out.append(f"{where}: path must be a relative path inside this repo")
            continue
        extra = sorted(set(b) - SUBMITTED)
        if extra:
            out.append(f"{where}: fields other than {', '.join(sorted(SUBMITTED))}: {', '.join(extra)}")
        if not REPO.match(repo):
            out.append(f"{where}: repo must be https://github.com/OWNER/REPO")
        if not SHA.match(commit):
            out.append(f"{where}: commit must be a full 40-hex commit id")
        if not HASH.match(b.get("sha256", "")):
            out.append(f"{where}: sha256 must be the module's 64-hex sha256")
        w = wasm_repo(b.get("wasm", ""))
        if not w:
            out.append(f"{where}: wasm must be a release asset (https://github.com/OWNER/REPO/releases/download/TAG/FILE) "
                       "or a file at a commit (https://raw.githubusercontent.com/OWNER/REPO/COMMIT/PATH)")
        elif REPO.match(repo) and w != repo.lower():
            out.append(f"{where}: wasm must be in the bot's own repo, {repo}")
    return out


def run(*cmd, cwd=None):
    subprocess.run(cmd, cwd=cwd, check=True)


def git(*args, cwd=ROOT):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True).stdout.strip()


def toolchain():
    return subprocess.run(["rustc", "--version"], capture_output=True, text=True).stdout.strip()


def cache_key(b, repo, commit):
    """What a module was built from, hashed: the same key means the same build inputs."""
    path = b.get("path", ".")
    if not repo:  # a bot in this repo: the last commit touching it or the SDK
        commit = git("log", "-1", "--format=%H", "--", path, "bots/sdk") or commit
    h = hashlib.sha256(json.dumps([repo, commit, path, toolchain()]).encode()).hexdigest()[:24]
    return f"{b['name']}-{h}"


def fetch(url, sha256, dest):
    """Download url to dest, refusing anything over MAX_WASM bytes or whose sha256 isn't sha256."""
    req = urllib.request.Request(url, headers={"User-Agent": "libaw-arena"})
    h, data = hashlib.sha256(), bytearray()
    with urllib.request.urlopen(req, timeout=60) as r:
        while chunk := r.read(1 << 16):
            data += chunk
            if len(data) > MAX_WASM:
                raise SystemExit(f"{url}: over {MAX_WASM >> 20} MiB")
            h.update(chunk)
    if h.hexdigest() != sha256:
        raise SystemExit(f"{url}: sha256 is {h.hexdigest()}, but bots.json says {sha256}")
    tmp = dest + ".part"
    open(tmp, "wb").write(data)
    os.replace(tmp, dest)


def build(b, work):
    """Returns (wasm path, repo, commit, cached) for one entry."""
    repo, commit, path = b.get("repo", ""), b.get("commit", ""), b.get("path", ".")
    out = os.path.join(ROOT, "bots", "out", b["name"] + ".wasm")
    if repo:  # a submitted module: download it, check its hash
        cached = os.path.join(ROOT, "bots", "cache", "sha256-" + b["sha256"] + ".wasm")
        hit = os.path.exists(cached) and hashlib.sha256(open(cached, "rb").read()).hexdigest() == b["sha256"]
        if not hit:
            os.makedirs(os.path.dirname(cached), exist_ok=True)
            fetch(b["wasm"], b["sha256"], cached)
        shutil.copyfile(cached, out)
        return out, repo, commit, hit
    shown_repo = repo or (os.environ.get("GITHUB_SERVER_URL", "") and f"{os.environ['GITHUB_SERVER_URL']}/{os.environ.get('GITHUB_REPOSITORY', '')}")
    shown_commit = commit or git("rev-parse", "HEAD")
    cached = os.path.join(ROOT, "bots", "cache", cache_key(b, repo, commit) + ".wasm")
    if os.path.exists(cached):
        shutil.copyfile(cached, out)
        return out, shown_repo, shown_commit, True
    crate = os.path.join(ROOT, path)
    target = os.path.join(work, "target-" + b["name"])
    run("cargo", "build", "--release", "--quiet", "--target", "wasm32-unknown-unknown",
        "--manifest-path", os.path.join(crate, "Cargo.toml"), "--target-dir", target)
    rel = os.path.join(target, "wasm32-unknown-unknown", "release")
    wasms = [f for f in os.listdir(rel) if f.endswith(".wasm")]
    if len(wasms) != 1:
        raise SystemExit(f"{b['name']}: expected one .wasm from the build, got {wasms}")
    os.makedirs(os.path.dirname(cached), exist_ok=True)
    shutil.copyfile(os.path.join(rel, wasms[0]), cached)
    shutil.copyfile(cached, out)
    return out, shown_repo, shown_commit, False


def main(argv):
    reg = json.load(open(os.path.join(ROOT, "bots.json")))
    bad = problems(reg)
    if bad:
        raise SystemExit("bots.json:\n  " + "\n  ".join(bad))
    if "--check" in argv:
        print(f"bots.json: {len(reg['open'])} open bots, ok")
        return
    only = None
    if "--only" in argv:
        only = set(argv[argv.index("--only") + 1].split(","))
    if "--changed-since" in argv:
        ref = argv[argv.index("--changed-since") + 1]
        old = subprocess.run(["git", "show", f"{ref}:bots.json"], cwd=ROOT, capture_output=True, text=True)
        before = {b["name"]: b for b in json.loads(old.stdout or "{}").get("open", [])}
        changed = {b["name"] for b in reg["open"] if before.get(b["name"]) != b}
        only = changed | {b["name"] for b in reg["open"] if not b.get("repo")}
        print("changed: " + (", ".join(sorted(changed)) or "none"))
        os.makedirs(os.path.join(ROOT, "bots", "out"), exist_ok=True)
        json.dump(sorted(changed), open(os.path.join(ROOT, "bots", "out", "changed.json"), "w"))
    work = os.path.join(ROOT, "bots", "work")
    os.makedirs(os.path.join(ROOT, "bots", "out"), exist_ok=True)
    os.makedirs(work, exist_ok=True)
    manifest = []
    for b in reg["open"]:
        if only and b["name"] not in only:
            continue
        wasm, repo, commit, hit = build(b, work)
        sha = hashlib.sha256(open(wasm, "rb").read()).hexdigest()
        manifest.append({"name": b["name"], "wasm": os.path.relpath(wasm, ROOT), "repo": repo, "commit": commit, "sha256": sha})
        print(f"{b['name']}: {repo or 'this repo'}@{commit[:12]} {'cached' if hit else ('downloaded' if b.get('repo') else 'built')} -> {os.path.relpath(wasm, ROOT)} {os.path.getsize(wasm)} bytes sha256 {sha[:16]}")
    json.dump(manifest, open(os.path.join(ROOT, "bots", "out", "manifest.json"), "w"), indent=1)


if __name__ == "__main__":
    main(sys.argv[1:])
