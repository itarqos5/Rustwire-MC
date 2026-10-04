import unittest
from check_reset_chat_fixtures import fixtures, inspect


def schema():
    return {'configuration': {'toClient': {'types': {
        'packet': ['container', [
            {'name': 'name', 'type': ['mapper', {'mappings': {'0x06': 'reset_chat'}}]},
            {'name': 'params', 'type': ['switch', {'compareTo': 'name', 'fields': {'reset_chat': 'packet_reset_chat'}}]}]],
        'packet_reset_chat': ['container', []]}}}}


class ResetChatAudit(unittest.TestCase):
    def test_all_supported_empty_layouts(self):
        for p in range(763, 777):
            self.assertEqual(inspect(schema() if p >= 766 else {}, p), [6] if p >= 766 else [])
        self.assertEqual(fixtures([(766, [6])]), '# protocol\tpacket_id\tbody_hex\n766\t6\t\n')

    def test_reject_wrong_presence(self):
        for p in (763, 764, 765):
            with self.assertRaises(ValueError): inspect(schema(), p)
        for p in range(766, 777):
            with self.assertRaises(ValueError): inspect({}, p)
        s = schema(); s['play'] = s.pop('configuration')
        with self.assertRaises(ValueError): inspect(s, 776)
        s = schema(); s['configuration']['toServer'] = s['configuration'].pop('toClient')
        with self.assertRaises(ValueError): inspect(s, 776)

    def test_reject_nonempty_or_redirected_body(self):
        s = schema(); s['configuration']['toClient']['types']['packet_reset_chat'][1].append({'name': 'extra', 'type': 'bool'})
        with self.assertRaises(ValueError): inspect(s, 776)
        s = schema(); s['configuration']['toClient']['types']['packet'][1][1]['type'][1]['fields']['reset_chat'] = 'other'
        with self.assertRaises(ValueError): inspect(s, 776)

    def test_reject_duplicate_or_moved_id(self):
        s = schema(); mapping = s['configuration']['toClient']['types']['packet'][1][0]['type'][1]['mappings']
        mapping['0x07'] = mapping.pop('0x06')
        with self.assertRaises(ValueError): inspect(s, 776)
        mapping['0x06'] = 'reset_chat'
        with self.assertRaises(ValueError): inspect(s, 776)


if __name__ == '__main__':
    unittest.main()
