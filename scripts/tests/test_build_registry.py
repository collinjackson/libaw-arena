import hashlib, os, sys, tempfile, unittest
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import build_registry as br  # noqa: E402
from build_registry import problems, wasm_repo  # noqa: E402

C1 = "a" * 40
REL = "https://github.com/Alice/mine/releases/download/v1.0/mine.wasm"
RAW = f"https://raw.githubusercontent.com/Alice/mine/{C1}/out/mine.wasm"
MINE = {"name": "mine", "repo": "https://github.com/Alice/mine", "commit": C1, "wasm": REL, "sha256": "c" * 64}
EX = {"name": "greedy", "repo": "", "commit": "", "path": "bots/greedy"}


def errs(*bots):
    return problems({"open": list(bots)})


class Entries(unittest.TestCase):
    def test_a_release_asset_or_a_file_at_a_commit(self):
        self.assertEqual(errs(EX, MINE), [])
        self.assertEqual(errs(dict(MINE, wasm=RAW)), [])

    def test_the_module_must_be_in_the_bots_own_repo(self):
        self.assertTrue(any("own repo" in e for e in errs(dict(MINE, wasm=REL.replace("Alice/mine", "mallory/x")))))
        self.assertEqual(errs(dict(MINE, wasm=REL.replace("Alice", "alice"))), [])  # GitHub names ignore case

    def test_other_urls_are_refused(self):
        for url in ["http://github.com/Alice/mine/releases/download/v1/mine.wasm",
                    "https://example.com/mine.wasm",
                    "https://github.com/Alice/mine/blob/main/mine.wasm",
                    "https://raw.githubusercontent.com/Alice/mine/main/mine.wasm",  # a branch moves; a commit doesn't
                    f"https://raw.githubusercontent.com/Alice/mine/{C1}/../x.wasm",
                    REL + "?x=1"]:
            self.assertIsNone(wasm_repo(url), url)
            self.assertTrue(errs(dict(MINE, wasm=url)), url)

    def test_needs_the_hash_and_the_source_commit(self):
        self.assertTrue(errs(dict(MINE, sha256="C" * 64)))
        self.assertTrue(errs(dict(MINE, sha256="")))
        self.assertTrue(errs(dict(MINE, commit="main")))

    def test_submitted_bots_are_never_built_here(self):
        self.assertTrue(any("path" in e for e in errs(dict(MINE, path="."))))
        self.assertTrue(any("wasm" in e for e in errs(dict(EX, wasm=REL))))


class Fetch(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.src = os.path.join(self.dir, "bot.wasm")
        with open(self.src, "wb") as f:
            f.write(b"\0asm\1\0\0\0")
        self.sha = hashlib.sha256(b"\0asm\1\0\0\0").hexdigest()
        self.url = "file://" + self.src

    def test_keeps_a_matching_module(self):
        dest = os.path.join(self.dir, "out.wasm")
        br.fetch(self.url, self.sha, dest)
        with open(dest, "rb") as f:
            self.assertEqual(f.read(), b"\0asm\1\0\0\0")

    def test_refuses_a_different_hash(self):
        dest = os.path.join(self.dir, "out.wasm")
        with self.assertRaises(SystemExit) as e:
            br.fetch(self.url, "0" * 64, dest)
        self.assertIn("sha256", str(e.exception))
        self.assertFalse(os.path.exists(dest))

    def test_refuses_an_oversized_module(self):
        old, br.MAX_WASM = br.MAX_WASM, 4
        try:
            with self.assertRaises(SystemExit):
                br.fetch(self.url, self.sha, os.path.join(self.dir, "out.wasm"))
        finally:
            br.MAX_WASM = old


class Gather(unittest.TestCase):
    def test_downloads_once_then_reuses_by_hash(self):
        root = tempfile.mkdtemp()
        os.makedirs(os.path.join(root, "bots", "out"))
        body = b"\0asm\1\0\0\0"
        sha = hashlib.sha256(body).hexdigest()
        calls = []

        def fake_fetch(url, want, dest):
            calls.append(url)
            with open(dest, "wb") as f:
                f.write(body)
        old = br.ROOT, br.fetch
        br.ROOT, br.fetch = root, fake_fetch
        try:
            for _ in range(2):
                out, repo, commit, hit = br.build(dict(MINE, sha256=sha), root)
            with open(out, "rb") as f:
                self.assertEqual(f.read(), body)
            self.assertEqual((repo, commit, hit, calls), (MINE["repo"], C1, True, [REL]))
        finally:
            br.ROOT, br.fetch = old


if __name__ == "__main__":
    unittest.main()
