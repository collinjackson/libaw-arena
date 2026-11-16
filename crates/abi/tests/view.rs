//! The VIEW answer decodes back to the same state, so a bot that brings libaw sees what the host
//! sees and picks from the same legal list.
#[test]
fn the_view_round_trips_through_postcard() {
    let v: serde_json::Value = serde_json::from_str(include_str!("../../../maps/skirmish.json")).unwrap();
    let s: libaw::State = if v.get("gameState").is_some() { libaw::awbw::state_from(&v) } else { serde_json::from_value(v).unwrap() };
    let b = arena_abi::answer(&s, &[], arena_abi::VIEW, 0);
    assert!(!b.is_empty());
    let back: libaw::State = postcard::from_bytes(&b).unwrap();
    assert_eq!(libaw::view::state_hash(&back), libaw::view::state_hash(&s));
    assert_eq!(libaw::actions::legal_actions(&back), libaw::actions::legal_actions(&s));
}
