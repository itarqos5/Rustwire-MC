import unittest
from verify_border_units import classify


class BorderDurationUnitTests(unittest.TestCase):
    def test_boundary(self):
        self.assertEqual(classify('// long 1000l', 'Util.getMillis:', 773)['duration_unit'], 'milliseconds')
        self.assertEqual(classify('TimeArgument.time:', 'lerpProgress:J', 774)['duration_unit'], 'ticks')

    def test_partial_or_mismatched_evidence_fails(self):
        for command, extent, protocol in [('', '', 773), ('// long 1000l', 'lerpProgress:J', 773),
                ('TimeArgument.time:', 'Util.getMillis:', 774), ('// long 1000l', 'Util.getMillis:', 774)]:
            with self.assertRaises(AssertionError):
                classify(command, extent, protocol)


if __name__ == '__main__':
    unittest.main()
