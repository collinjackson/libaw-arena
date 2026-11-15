#!/usr/bin/env python3
"""Build every open bot in bots.json at its pinned commit into bots/out/NAME.wasm, and write
bots/out/manifest.json: [{name, wasm, repo, commit, sha256}], which `arena round-robin
--manifest` reads.

An entry is {"name", "repo", "commit", "path"}: repo is https://github.com/OWNER/REPO and commit
a full 40-hex commit id; path is the bot's crate inside the repo ("." by default). An empty repo
and commit mean a bot in this repo, built at this repo's own commit.

Each module is built once and kept in bots/cache/, named for what it was built from (repo,
commit, path and the Rust toolchain), so later runs reuse it instead of compiling again; the
workflows keep bots/cache between runs. A bot in this repo is keyed by the last commit that
touched it or the SDK.

Usage: scripts/build_registry.py [--only NAME,...] [--changed-since REF] [--check]
  --check validates bots.json without building.
  --changed-since REF builds the bots whose entries differ from bots.json at git REF, plus this
  repo's own example bots to play them against (a pull request's check).
"""
import hashlib, json, os, re, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NAME = re.compile(r"^[a-z0-9][a-z0-9_-]{0,31}$")
REPO = re.compile(r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
SHA = re.compile(r"^[0-9a-f]{40}$")
PATH = re.compile(r"^(\.|[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)*)$")


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
        if repo or commit:
            if not REPO.match(repo):
                out.append(f"{where}: repo must be https://github.com/OWNER/REPO")
            if not SHA.match(commit):
                out.append(f"{where}: commit must be a full 40-hex commit id")
        path = b.get("path", ".")
        if not PATH.match(path) or ".." in path.split("/"):
            out.append(f"{where}: path must be a relative path inside the repo")
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


def build(b, work):
    """Returns (wasm path, repo, commit, cached) for one entry."""
    repo, commit, path = b.get("repo", ""), b.get("commit", ""), b.get("path", ".")
    out = os.path.join(ROOT, "bots", "out", b["name"] + ".wasm")
    shown_repo = repo or (os.environ.get("GITHUB_SERVER_URL", "") and f"{os.environ['GITHUB_SERVER_URL']}/{os.environ.get('GITHUB_REPOSITORY', '')}")
    shown_commit = commit or git("rev-parse", "HEAD")
    cached = os.path.join(ROOT, "bots", "cache", cache_key(b, repo, commit) + ".wasm")
    if os.path.exists(cached):
        shutil.copyfile(cached, out)
        return out, shown_repo, shown_commit, True
    if repo:
        src = os.path.join(work, b["name"])
        shutil.rmtree(src, ignore_errors=True)
        run("git", "init", "-q", src)
        run("git", "fetch", "-q", "--depth", "1", repo, commit, cwd=src)
        run("git", "checkout", "-q", "FETCH_HEAD", cwd=src)
    else:
        src = ROOT
    crate = os.path.join(src, path)
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
        print(f"{b['name']}: {repo or 'this repo'}@{commit[:12]} {'cached' if hit else 'built'} -> {os.path.relpath(wasm, ROOT)} {os.path.getsize(wasm)} bytes sha256 {sha[:16]}")
    json.dump(manifest, open(os.path.join(ROOT, "bots", "out", "manifest.json"), "w"), indent=1)


if __name__ == "__main__":
    main(sys.argv[1:])
