import json, os, sys, unittest
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from automerge import eligible, owner  # noqa: E402

C1, C2 = "a" * 40, "b" * 40
EX = {"name": "greedy", "repo": "", "commit": "", "path": "bots/greedy"}
H1 = "c" * 64
MINE = {"name": "mine", "repo": "https://github.com/Alice/mine", "commit": C1,
        "wasm": "https://github.com/Alice/mine/releases/download/v1/mine.wasm", "sha256": H1}
BOB = {"name": "bobs", "repo": "https://github.com/bob/bobs", "commit": C1,
       "wasm": "https://github.com/bob/bobs/releases/download/v1/bobs.wasm", "sha256": H1}
OK = {"ok": True, "changed": ["mine"], "reasons": []}
NONE = {"ok": True, "changed": [], "reasons": []}


def reg(*bots):
    return {"about": "the registry", "open": list(bots)}


def text(d):
    return json.dumps(d, indent=2)


def check(base, head, author, v=OK, files=("bots.json",), adder=lambda n: None):
    return eligible(base, head if isinstance(head, str) else text(head), author, list(files), v, adder)


class Adding(unittest.TestCase):
    def test_owner_adds_their_bot(self):
        self.assertEqual(check(reg(EX), reg(EX, MINE), "alice"), (True, []))

    def test_one_bot_per_pull_request(self):
        ok, why = check(reg(EX), reg(EX, MINE, dict(MINE, name="mine2")), "alice", {"ok": True, "changed": ["mine", "mine2"]})
        self.assertFalse(ok)
        self.assertTrue(any("one bot per pull request" in w for w in why))

    def test_no_extra_fields(self):
        ok, why = check(reg(EX), reg(EX, dict(MINE, homepage="https://x")), "alice")
        self.assertFalse(ok)
        self.assertTrue(any("homepage" in w for w in why))

    def test_needs_repo_and_commit(self):
        self.assertFalse(check(reg(EX), reg(EX, dict(MINE, repo="", commit="")), "alice")[0])

    def test_invalid_json(self):
        ok, why = check(reg(EX), '{"open": [', "alice")
        self.assertFalse(ok)
        self.assertIn("valid JSON", why[-1])

    def test_other_top_level_changes(self):
        self.assertFalse(check(reg(EX), dict(reg(EX, MINE), about="mine now"), "alice")[0])

    def test_other_files(self):
        self.assertFalse(check(reg(EX), reg(EX, MINE), "alice", files=("bots.json", "crates/arena/src/referee.rs"))[0])

    def test_a_dirty_check_blocks_it(self):
        bad = {"ok": False, "changed": ["mine"], "reasons": ["standard/game-01: 3 strike(s): illegal action"]}
        ok, why = check(reg(EX), reg(EX, MINE), "alice", bad)
        self.assertFalse(ok)
        self.assertTrue(any("illegal action" in w for w in why))

    def test_the_check_must_cover_the_change(self):
        self.assertFalse(check(reg(EX), reg(EX, MINE), "alice", NONE)[0])


class Updating(unittest.TestCase):
    def test_owner_updates_the_commit(self):
        self.assertTrue(check(reg(EX, MINE), reg(EX, dict(MINE, commit=C2)), "Alice")[0])

    def test_someone_else_cant(self):
        ok, why = check(reg(EX, MINE), reg(EX, dict(MINE, commit=C2)), "mallory")
        self.assertFalse(ok)
        self.assertIn("doesn't own", why[0])

    def test_or_point_its_name_at_their_own_repo(self):
        moved = dict(MINE, repo="https://github.com/mallory/x", commit=C2)
        self.assertFalse(check(reg(EX, MINE), reg(EX, moved), "mallory")[0])

    def test_example_bots_need_a_maintainer(self):
        self.assertFalse(check(reg(EX), reg(dict(EX, path="bots/random")), "alice", {"ok": True, "changed": ["greedy"]})[0])


class Removing(unittest.TestCase):
    def test_the_repo_owner_can(self):
        self.assertTrue(check(reg(EX, MINE), reg(EX), "alice", NONE)[0])

    def test_whoever_added_it_can(self):
        self.assertTrue(check(reg(EX, BOB), reg(EX), "carol", NONE, adder=lambda n: "Carol")[0])

    def test_anyone_else_cant(self):
        ok, why = check(reg(EX, BOB), reg(EX), "mallory", NONE, adder=lambda n: "carol")
        self.assertFalse(ok)
        self.assertIn("whoever added it (carol)", why[0])

    def test_example_bots_need_a_maintainer(self):
        self.assertFalse(check(reg(EX, MINE), reg(MINE), "alice", NONE, adder=lambda n: "alice")[0])


class Parsing(unittest.TestCase):
    def test_owner(self):
        self.assertEqual(owner("https://github.com/Alice/mine"), "alice")
        self.assertEqual(owner(""), "")


if __name__ == "__main__":
    unittest.main()
