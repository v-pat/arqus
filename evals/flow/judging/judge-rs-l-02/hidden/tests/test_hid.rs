use fixture::context::Context;
use fixture::pipeline::Pipeline;
#[test] fn test_hid() {
    let mut c = Context::new();
    Pipeline::run(&mut c);
    Pipeline::run(&mut c);
    assert_eq!(c.step_count, 2);
}
