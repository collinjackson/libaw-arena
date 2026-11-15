import os, sys, unittest
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from automerge import eligible, owner  # noqa: E402

C1, C2 = "a" * 40, "b" * 40
EX = {"name": "greedy", "repo": "", "commit": "", "path": "bots/greedy"}
MINE = {"name": "mine", "repo": "https://github.com/Alice/mine", "commit": C1, "path": "."}
OK = {"ok": True, "changed": ["mine"], "reasons": []}


def reg(*bots):
    return {"open": list(bots)}


class Eligible(unittest.TestCase):
    def test_owner_adds_their_bot(self):
        self.assertEqual(eligible(reg(EX), reg(EX, MINE), "alice", ["bots.json"], OK), (True, []))

    def test_owner_updates_the_commit(self):
        newer = dict(MINE, commit=C2)
        self.assertTrue(eligible(reg(EX, MINE), reg(EX, newer), "Alice", ["bots.json"], OK)[0])

    def test_someone_else_cant_update_it(self):
        newer = dict(MINE, commit=C2)
        ok, why = eligible(reg(EX, MINE), reg(EX, newer), "mallory", ["bots.json"], OK)
        self.assertFalse(ok)
        self.assertIn("doesn't own", why[0])

    def test_or_point_its_name_at_their_own_repo(self):
        moved = dict(MINE, repo="https://github.com/mallory/x", commit=C2)
        self.assertFalse(eligible(reg(EX, MINE), reg(EX, moved), "mallory", ["bots.json"], OK)[0])

    def test_example_bots_need_a_maintainer(self):
        changed = dict(EX, path="bots/random")
        self.assertFalse(eligible(reg(EX), reg(changed), "alice", ["bots.json"], {"ok": True, "changed": ["greedy"]})[0])

    def test_other_files_need_a_maintainer(self):
        self.assertFalse(eligible(reg(EX), reg(EX, MINE), "alice", ["bots.json", "crates/arena/src/referee.rs"], OK)[0])

    def test_a_dirty_check_blocks_it(self):
        bad = {"ok": False, "changed": ["mine"], "reasons": ["standard/game-01: 3 strike(s): illegal action"]}
        ok, why = eligible(reg(EX), reg(EX, MINE), "alice", ["bots.json"], bad)
        self.assertFalse(ok)
        self.assertIn("illegal action", why[0])

    def test_the_check_must_cover_the_change(self):
        self.assertFalse(eligible(reg(EX), reg(EX, MINE), "alice", ["bots.json"], {"ok": True, "changed": []})[0])

    def test_owner_parsing(self):
        self.assertEqual(owner("https://github.com/Alice/mine"), "alice")
        self.assertEqual(owner(""), "")


if __name__ == "__main__":
    unittest.main()
