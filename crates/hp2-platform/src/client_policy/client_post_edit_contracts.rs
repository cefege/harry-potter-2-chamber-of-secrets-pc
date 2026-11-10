//! PostEditChange float, callback order, and error contracts.

use super::{
    Hp2PlatformClientAction as Action, Hp2PlatformClientField as Field,
    Hp2PlatformClientLeaf as Leaf, Hp2PlatformClientServices as Services,
    Hp2PlatformClientValue as Value, run,
};
use std::ffi::c_void;
use std::ptr;

const SUPER_ERROR: i32 = 101;
const READ_ERROR: i32 = -202;
const WRITE_ERROR: i32 = 303;
const UNEXPECTED: i32 = 404;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Super,
    Read,
    Write(u32),
    Unexpected,
}

struct Fixture {
    input: f32,
    output: Option<u32>,
    super_status: i32,
    read_status: i32,
    write_status: i32,
    events: Vec<Event>,
}

impl Fixture {
    fn new(input: f32) -> Self {
        Self {
            input,
            output: None,
            super_status: 0,
            read_status: 0,
            write_status: 0,
            events: Vec::new(),
        }
    }
}

// Each callback receives the sole pointer derived from the live Fixture in
// `invoke`. The invocation is synchronous and single-threaded. No reference to
// the fixture is held by the caller while `run` or a callback executes. Callback
// references end before return. The PostEditChange action does not reenter this
// fixture. All outputs are valid for their documented call and are written only
// on success. Unexpected operations return an error instead of panicking across
// the extern boundary. Allocation failure follows Rust's normal abort behavior.
unsafe extern "C" fn call(
    context: *mut c_void,
    leaf: Leaf,
    object: *mut c_void,
    other: *mut c_void,
    integer: i32,
    output: *mut Value,
) -> i32 {
    // SAFETY: `invoke` supplies the live, exclusively accessed fixture pointer.
    let fixture = unsafe { &mut *context.cast::<Fixture>() };
    if matches!(leaf, Leaf::SuperPostEditChange)
        && object.is_null()
        && other.is_null()
        && integer == 0
        && !output.is_null()
    {
        fixture.events.push(Event::Super);
        fixture.super_status
    } else {
        fixture.events.push(Event::Unexpected);
        UNEXPECTED
    }
}

unsafe extern "C" fn read_float(context: *mut c_void, field: Field, output: *mut f32) -> i32 {
    // SAFETY: see the fixture invariant above; the output is supplied by run.
    let fixture = unsafe { &mut *context.cast::<Fixture>() };
    if !matches!(field, Field::UIScale) || output.is_null() {
        fixture.events.push(Event::Unexpected);
        return UNEXPECTED;
    }
    fixture.events.push(Event::Read);
    if fixture.read_status == 0 {
        // SAFETY: successful `read_real` passes a writable, aligned f32 slot.
        unsafe { output.write(fixture.input) };
    }
    fixture.read_status
}

unsafe extern "C" fn write_float(context: *mut c_void, field: Field, value: f32) -> i32 {
    // SAFETY: see the fixture invariant above.
    let fixture = unsafe { &mut *context.cast::<Fixture>() };
    if !matches!(field, Field::UIScale) {
        fixture.events.push(Event::Unexpected);
        return UNEXPECTED;
    }
    fixture.events.push(Event::Write(value.to_bits()));
    if fixture.write_status == 0 {
        fixture.output = Some(value.to_bits());
    }
    fixture.write_status
}

unsafe extern "C" fn read_integer(context: *mut c_void, _: Field, _: *mut i32) -> i32 {
    // SAFETY: see the fixture invariant above.
    let fixture = unsafe { &mut *context.cast::<Fixture>() };
    fixture.events.push(Event::Unexpected);
    UNEXPECTED
}

unsafe extern "C" fn write_integer(context: *mut c_void, _: Field, _: i32) -> i32 {
    // SAFETY: see the fixture invariant above.
    let fixture = unsafe { &mut *context.cast::<Fixture>() };
    fixture.events.push(Event::Unexpected);
    UNEXPECTED
}

fn invoke(fixture: &mut Fixture) -> Result<i32, i32> {
    // Derive one pointer for this persistent record. Do not derive additional
    // pointers from references or access the record during the policy call.
    let context = ptr::addr_of_mut!(*fixture).cast::<c_void>();
    let services = Services {
        context,
        call,
        read_integer,
        write_integer,
        read_float,
        write_float,
    };
    // SAFETY: the table and fixture satisfy the synchronous callback contract
    // above; this action passes no native object tokens to these callbacks.
    unsafe { run(services, Action::PostEditChange) }
}

#[test]
fn post_edit_float_matrix_preserves_exact_callback_order() {
    // Expected outcomes are explicit, not a copy of the production predicate.
    let cases: &[(u32, bool)] = &[
        (0x0000_0000, true),  // +0
        (0x8000_0000, true),  // -0
        (0x0000_0001, false), // smallest positive subnormal
        (0x8000_0001, true),  // smallest negative subnormal
        (1.0_f32.to_bits(), false),
        ((-1.0_f32).to_bits(), true),
        (f32::MAX.to_bits(), false),
        (f32::MIN.to_bits(), true),
        (f32::INFINITY.to_bits(), false),
        (f32::NEG_INFINITY.to_bits(), true),
        (0x7fc0_0001, true), // positive quiet NaN, nonzero payload
        (0xffc0_0002, true), // negative quiet NaN, nonzero payload
        (0x7f80_0001, true), // positive signaling NaN representation
    ];
    for &(bits, writes) in cases {
        let mut fixture = Fixture::new(f32::from_bits(bits));
        let result = invoke(&mut fixture);
        let mut expected = vec![Event::Super, Event::Read];
        if writes {
            expected.push(Event::Write(1.0_f32.to_bits()));
        }
        assert_eq!(result, Ok(0), "input bits {bits:#010x}");
        assert_eq!(fixture.events, expected, "input bits {bits:#010x}");
        assert_eq!(fixture.output, writes.then_some(1.0_f32.to_bits()));
        assert_eq!(fixture.input.to_bits(), bits, "input record changed");
    }
}

#[test]
fn post_edit_propagates_first_error_without_later_callbacks() {
    let mut fixture = Fixture::new(0.0);
    fixture.super_status = SUPER_ERROR;
    fixture.read_status = READ_ERROR;
    fixture.write_status = WRITE_ERROR;
    assert_eq!(invoke(&mut fixture), Err(SUPER_ERROR));
    assert_eq!(fixture.events, [Event::Super]);
    assert_eq!(fixture.output, None);

    let mut fixture = Fixture::new(0.0);
    fixture.read_status = READ_ERROR;
    fixture.write_status = WRITE_ERROR;
    assert_eq!(invoke(&mut fixture), Err(READ_ERROR));
    assert_eq!(fixture.events, [Event::Super, Event::Read]);
    assert_eq!(fixture.output, None);

    let mut fixture = Fixture::new(-0.0);
    fixture.write_status = WRITE_ERROR;
    assert_eq!(invoke(&mut fixture), Err(WRITE_ERROR));
    assert_eq!(
        fixture.events,
        [Event::Super, Event::Read, Event::Write(1.0_f32.to_bits())]
    );
    assert_eq!(fixture.output, None);

    // A write failure must not matter when a positive value avoids the write.
    let mut fixture = Fixture::new(f32::INFINITY);
    fixture.write_status = WRITE_ERROR;
    assert_eq!(invoke(&mut fixture), Ok(0));
    assert_eq!(fixture.events, [Event::Super, Event::Read]);
    assert_eq!(fixture.output, None);
}
