"""Unit checks for the offline release-oracle result verifier (no Java needed)."""
import unittest
import validate_scoreboard_oracles as oracle


def packets(protocol):
    """Small synthetic observation report, independent of a game process."""
    expected, rejected = oracle.packet_cases(protocol)
    rows = {name: "ERROR DecoderException: malformed" if name in rejected else "OK" for name in expected}
    for n in (-128, -1, 0, 18, 19, 127, 128):
        value = str(-128 if n == 128 else n) if protocol == 763 else (
            oracle.DISPLAY[n] if 0 <= n < 19 else "LIST")
        rows[f"display_slot_{n}"] = f"[slot={value},objective=sample]"
    for prefix, values in (("objective_action", (-128, -1, 1, 3, 127)),
                           ("team_mode", (-128, -1, 1, 5, 127))):
        for n in values:
            rows[f"{prefix}_{n}"] = f"[action={n}]"
    for n in (-1, 0, 15, 16, 20, 21, 22):
        name = f"team_params_color_{n}"
        if name in rejected:
            continue
        color = (oracle.COLORS[n] if 0 <= n < 16 else "BLACK") if protocol == 776 else oracle.FORMATTING[n]
        rules = 'visibility="unknownVisibility",collision="unknownCollision"' if protocol < 770 else 'visibility=ALWAYS,collision=ALWAYS'
        rows[name] = f"[color={color},{rules},flags=-1]"
    rows["render_0"] = "[render=INTEGER,other=0]"
    rows["render_1"] = "[render=HEARTS,other=0]"
    if protocol < 765:
        rows["score_action_0"] = "[objective=null,value=-2147483648,other=0]"
    else:
        for n in (-2147483648, -1, 0, 2147483647):
            rows[f"score_value_{n}"] = f"[value={n},other=0]"
        rows["reset_no_objective"] = "[objective=null]"
        rows["reset_empty_objective"] = '[objective=""]'
        for n, name in enumerate(("BlankFormat", "StyledFormat", "FixedFormat")):
            rows[f"objective_format_{n}"] = name
        rows["style_root_10"] = "StyledFormat"
    return rows


def render(rows):
    return "\n".join(f"{key} {value}" for key, value in rows.items())


class ScoreboardOracleVerifier(unittest.TestCase):
    def test_all_release_case_sets_and_valid_observations(self):
        for protocol in range(763, 777):
            with self.subTest(protocol=protocol):
                result = oracle.verify_packets(render(packets(protocol)), protocol)
                expected = 46 if protocol < 765 else 57 if protocol < 770 else 55
                self.assertEqual(result["probes"], expected)

    def test_missing_case_rejects(self):
        rows = packets(776)
        del rows["display_slot_0"]
        with self.assertRaises(AssertionError):
            oracle.verify_packets(render(rows), 776)

    def test_duplicate_case_rejects(self):
        rows = packets(763)
        with self.assertRaises(AssertionError):
            oracle.verify_packets(render(rows) + "\nrender_0 " + rows["render_0"], 763)

    def test_wrong_acceptance_rejects(self):
        rows = packets(765)
        rows["style_root_8"] = "StyledFormat"
        with self.assertRaises(AssertionError):
            oracle.verify_packets(render(rows), 765)

    def test_harness_failure_is_not_expected_decode_rejection(self):
        rows = packets(765)
        rows["style_root_8"] = "ERROR NoSuchMethodException: missing API"
        with self.assertRaises(AssertionError):
            oracle.verify_packets(render(rows), 765)

    def test_wrong_color_fallback_or_flag_normalization_rejects(self):
        for old, new in (("BLACK", "WHITE"), ("flags=-1]", "flags=3]"),
                         ("visibility=ALWAYS", "visibility=NEVER")):
            rows = packets(776)
            rows["team_params_color_-1"] = rows["team_params_color_-1"].replace(old, new)
            with self.subTest(change=new), self.assertRaises(AssertionError):
                oracle.verify_packets(render(rows), 776)

    def test_empty_and_duplicate_domain_reports_reject(self):
        with self.assertRaises(AssertionError):
            oracle.verify_domain("", 776)
        with self.assertRaises(AssertionError):
            oracle.domain_lines("formatting=[]\nformatting=[]")


if __name__ == "__main__":
    unittest.main()
