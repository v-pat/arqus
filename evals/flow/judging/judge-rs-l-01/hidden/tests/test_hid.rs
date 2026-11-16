use fixture::registry::Registry;
use fixture::plugin::Plugin;
struct Mock2;
impl Plugin for Mock2 { fn name(&self) -> &str { "m2" } }
#[test] fn test_hid() {
    let mut r = Registry::new();
    r.register(Box::new(Mock2));
    assert_eq!(r.plugins[0].name(), "m2");
}
