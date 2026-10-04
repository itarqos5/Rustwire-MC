#!/usr/bin/env python3
"""Report client-role API bindings, not payload or gameplay conformance.

Outgoing entrypoints were manually traced at the transfer-login checkpoint.
The generator guards catalog inventory and incoming name dispatch; source
references are navigation aids, not semantic proof. See the coverage audit.
"""
import argparse
import collections
import pathlib
import re
ROOT = pathlib.Path(__file__).resolve().parents[1]

def inputs():
    catalogs = {p: (ROOT / f'src/catalog/p{p}.rs').read_text() for p in range(763, 777)}
    sources = {str(p.relative_to(ROOT)): p.read_text() for p in (ROOT / 'src').rglob('*.rs') if 'catalog' not in p.parts}
    return (catalogs, sources)

def records(catalogs, sources):
    if set(catalogs) != set(range(763, 777)):
        raise ValueError('protocol family inventory')
    api = {}

    def add(names, module, symbol, kind='outbound encoder'):
        for name in names.split():
            api[name] = (module, symbol, kind)
    for module in ['client_control', 'editing', 'world_edit']:
        for typ, name in re.findall('\\w+\\((\\w+)\\)\\s*=>\\s*"([a-z_]+)"', sources[f'src/packet/{module}.rs']):
            add(name, module, typ + '::packet')
    add('teleport_confirm', '', 'PositionSync::acknowledgement')
    add('message_acknowledgement', 'chat', 'message_acknowledgement')
    add('chat_command', 'chat', 'unsigned_command (766+); signed::SignedChatCommand::encode (763-765)')
    add('chat_command_signed', 'chat', 'signed::SignedChatCommand::encode')
    add('chat_message', 'chat', 'UnsignedChatMessage::encode; signed::SignedChatMessage::encode')
    add('chat_session_update', 'chat', 'signed::session_update')
    add('client_command', 'interact', 'ClientCommand::encode')
    add('settings', '', 'ClientSettings::encode')
    add('tab_complete', 'commands', 'CommandSuggestionRequest::packet')
    add('enchant_item', 'inventory', 'ContainerButtonClick::packet')
    add('window_click', 'inventory', 'ContainerClick::packet (through769); HashedContainerClick::packet (770+)')
    add('close_window', 'inventory', 'CloseContainer::packet')
    add('custom_payload', 'custom_payload', 'CustomPayload::packet / CustomPayloadRef::packet')
    add('use_entity attack', 'interact', 'UseEntity::encode')
    add('position position_look look flying', 'movement', 'PlayerMovement::packet')
    add('vehicle_move', 'movement', 'VehicleMovement::packet')
    add('craft_recipe_request', 'recipe', 'CraftRecipeRequest::packet')
    add('abilities', 'interact', 'player_abilities')
    add('block_dig', 'interact', 'Digging::encode')
    add('entity_action', 'interact', 'EntityAction::encode')
    add('steer_vehicle player_input', 'interact', 'PlayerInput::encode')
    add('recipe_book', 'recipe', 'RecipeBookChangeSettings::packet')
    add('displayed_recipe', 'recipe', 'DisplayedRecipe::packet')
    add('resource_pack_receive', 'common', 'ResourcePackOffer::response')
    add('advancement_tab', 'advancements', 'AdvancementTab::packet')
    add('select_trade', 'inventory', 'SelectTrade::packet')
    add('held_item_slot', 'inventory', 'SetSelectedSlot::packet (serverbound i16; body encode is clientbound)')
    add('arm_animation', 'interact', 'swing_arm')
    add('block_place', 'interact', 'UseBlock::encode')
    add('use_item', 'interact', 'UseItem::encode')
    add('chunk_batch_received', 'common', 'chunk_batch_received')
    add('ping_request', 'server_metadata', 'PingRequest::packet')
    add('debug_sample_subscription', 'debug', 'DebugSampleSubscription::packet')
    add('debug_subscription_request', 'debug', 'DebugSubscriptionRequest::packet')
    add('tick_end', 'interact', 'tick_end')
    add('player_loaded', 'world_state', 'PlayerLoaded::packet')
    add('set_test_block', 'game_test', 'SetTestBlock::packet')
    add('test_instance_block_action', 'game_test', 'TestInstanceBlockAction::packet')
    add('custom_click_action', 'dialog', 'CustomClickAction::packet')
    add('accept_code_of_conduct', 'common', 'accept_code_of_conduct')
    add('set_game_rule', 'game_rules', 'SetGameRules::packet')
    add('login_start', '', 'login_start; Connection::start_login / start_transfer_login')
    add('encryption_begin', '@connection', 'complete_encryption (crypto feature)')
    add('login_plugin_response', '@connection', 'answer_login_plugin')
    add('cookie_response', '@connection', 'answer_cookie')
    add('select_known_packs', '@connection', 'select_known_packs')
    add('login_acknowledged finish_configuration configuration_acknowledged keep_alive pong', '@connection', 'next_event', 'automatic control reply')
    control = {('Login', 'compress'): 'Compression', ('Login', 'encryption_begin'): 'EncryptionRequested', ('Login', 'success'): 'LoginSuccess', ('Login', 'login_plugin_request'): 'LoginPluginRequest', ('Configuration', 'finish_configuration'): 'Ready', ('Configuration', 'registry_data'): 'Registry', ('Configuration', 'select_known_packs'): 'KnownPacks', ('Play', 'login'): 'Joined', ('Play', 'start_configuration'): 'Reconfigure', ('Play', 'position'): 'Position'}
    for st in ['Login', 'Configuration', 'Play']:
        control[st, 'cookie_request'] = 'CookieRequest'
        control[st, 'disconnect'] = 'Disconnected'
        control[st, 'kick_disconnect'] = 'Disconnected'
    for st in ['Configuration', 'Play']:
        control[st, 'keep_alive'] = 'KeepAlive'
        control[st, 'ping'] = 'Ping'
    records = collections.OrderedDict()
    for protocol in range(763, 777):
        raw = catalogs[protocol]
        if raw.count('PacketInfo {') != len(re.findall('state: State::(\\w+),\\s*direction: Direction::(\\w+),\\s*id: (\\d+),\\s*name: "([^"]+)"', raw)):
            raise ValueError('unrecognized catalog entry')
        seen_names = set()
        seen_ids = set()
        for state, direction, id_, name in re.findall('state: State::(\\w+),\\s*direction: Direction::(\\w+),\\s*id: (\\d+),\\s*name: "([^"]+)"', raw):
            key = (state, direction, name)
            if key in seen_names or (state, direction, int(id_)) in seen_ids:
                raise ValueError('duplicate catalog identity')
            seen_names.add(key)
            seen_ids.add((state, direction, int(id_)))
            if key not in records:
                if state == 'Handshake':
                    if name == 'legacy_server_list_ping':
                        route = 'unsupported historical unframed protocol'
                        symbol = None
                        source = 'src/catalog/p' + str(protocol) + '.rs'
                    elif name == 'set_protocol':
                        route = 'outbound encoder'
                        symbol = 'packet::handshake / handshake_with_intent'
                        source = 'src/packet.rs'
                    else:
                        raise ValueError('unknown handshake packet')
                elif state == 'Status':
                    if (direction, name) not in [('Clientbound', 'server_info'), ('Clientbound', 'ping'), ('Serverbound', 'ping_start'), ('Serverbound', 'ping')]:
                        raise ValueError('unknown status packet')
                    route = 'client status request/response flow'
                    symbol = 'Connection::status'
                    source = 'src/connection.rs'
                elif direction == 'Clientbound':
                    if (state, name) in control:
                        route = 'connection control/disconnect'
                        symbol = 'Connection::next_event -> Event::' + control[state, name]
                        source = 'src/connection.rs'
                    else:
                        if state == 'Login':
                            raise ValueError('unknown login control packet')
                        if state == 'Configuration' and name not in ['custom_payload', 'resource_pack_send', 'feature_flags', 'tags', 'remove_resource_pack', 'add_resource_pack', 'reset_chat', 'store_cookie', 'transfer', 'custom_report_details', 'server_links', 'clear_dialog', 'show_dialog', 'code_of_conduct']:
                            raise ValueError('missing configuration dispatch')
                        route = 'typed incoming dispatcher'
                        symbol = 'DecodedPacket::decode_with_context'
                        source = 'src/packet/typed.rs'
                        if not any(('"' + name + '"' in sources[path] for path in ['src/packet/typed.rs', 'src/packet/common.rs', 'src/packet/dialog.rs'])):
                            raise ValueError('missing incoming dispatch binding: ' + name)
                        if name in ['map_chunk', 'update_light', 'chunk_biomes']:
                            route = 'typed incoming with caller dimension context'
                else:
                    allowed = ['Play']
                    if name in ['settings', 'custom_payload', 'resource_pack_receive', 'keep_alive', 'pong', 'custom_click_action']:
                        allowed = ['Configuration', 'Play']
                    elif name in ['login_start', 'encryption_begin', 'login_plugin_response', 'login_acknowledged']:
                        allowed = ['Login']
                    elif name == 'cookie_response':
                        allowed = ['Login', 'Configuration', 'Play']
                    elif name in ['select_known_packs', 'finish_configuration', 'accept_code_of_conduct']:
                        allowed = ['Configuration']
                    if state not in allowed:
                        raise ValueError('outgoing API state mismatch: '+state+':'+name)
                    if name not in api:
                        raise ValueError('missing outgoing API binding: ' + name)
                    module, symbol, route = api[name]
                    if module == '@connection':
                        symbol = 'Connection::' + symbol
                        source = 'src/connection.rs'
                    else:
                        source = 'src/packet' + ('/' + module if module else '') + '.rs'
                        symbol = 'packet::' + (module + '::' if module else '') + symbol
                records[key] = {'state': state, 'direction': direction, 'name': name, 'protocol_ids': [], 'route': route, 'api': symbol}
            records[key]['protocol_ids'].append({'protocol': protocol, 'id': int(id_)})
    return list(records.values())

def render(rows):
    text = '# state\tdirection\tname\tprotocol:packet_id\troute\tpublic_api\n'
    for row in rows:
        pairs = ','.join((f"{p['protocol']}:{p['id']}" for p in row['protocol_ids']))
        text += '\t'.join([row['state'], row['direction'], row['name'], pairs, row['route'], row['api'] or '-']) + '\n'
    return text

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    rows = records(*inputs())
    result = render(rows)
    path = ROOT / 'docs/packet-api-coverage.tsv'
    if args.check:
        if path.read_text() != result:
            raise SystemExit('Packet API coverage snapshot is stale')
    else:
        path.write_text(result)
    print(f"Verified {len(rows)} state/direction/name bindings and {sum((len(r['protocol_ids']) for r in rows))} versioned rows; not a wire-conformance claim")
if __name__ == '__main__':
    main()
