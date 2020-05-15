use lazy_static::lazy_static;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe};
use std::sync::{mpsc, mpsc::Sender, Mutex};

// A task for the test runner, and a channel to use to send the result back to the test thread.
struct TestTask {
    function: Box<dyn Send + UnwindSafe + FnOnce() -> Box<dyn Any + Send + 'static>>,
    tx: Sender<std::thread::Result<Box<dyn Any + Send + 'static>>>,
}

lazy_static! {
    static ref RUNNER: Mutex<Sender<TestTask>> = {
        let (tx, rx) = mpsc::channel::<TestTask>();
        std::thread::spawn(move || loop {
            if let Ok(task) = rx.recv() {
                let result = catch_unwind(task.function);
                task.tx
                    .send(result)
                    .expect("failed to reply with task status");
            } else {
                break;
            }
        });
        Mutex::new(tx)
    };
}

// Panics using a dynamically-typed value, trying to make it look good.
//
// Without this function, any panic from an inner test would just be reported as Box<Any>, which is
// obviously not super useful.
fn nice_panic(err: Box<dyn Any + Send>) -> ! {
    if let Some(err) = err.downcast_ref::<String>() {
        // panic!("foo {}", bar);
        panic!("{}", err);
    } else if let Some(err) = err.downcast_ref::<&str>() {
        // panic("baz")
        panic!("{}", err);
    } else {
        // panic(some_random_variable)
        panic!(err);
    }
}

pub fn run_test<F, T>(function: F) -> T
where
    F: FnOnce() -> T,
    F: Send + UnwindSafe + 'static,
    T: Any + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    RUNNER
        .lock()
        .unwrap()
        .send(TestTask {
            function: Box::new(move || Box::new(function())),
            tx,
        })
        .unwrap();
    match rx.recv().expect("Failed to receive") {
        // The test panicked, and this is the thing we got.
        Err(err) => nice_panic(err),
        // The test didn't panic, though it still might have failed.
        Ok(result) => *result
            .downcast::<T>()
            .expect("Got back something with a type we didn't expect"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success() {
        run_test(|| ())
    }

    #[test]
    fn success_with_result() -> Result<(), i32> {
        run_test(|| -> Result<(), i32> { Ok(()) })
    }

    // We test for single-argument and formatted panic, because the former will pass a &str and the
    // latter actually allocates for a String.

    #[test]
    #[should_panic(expected = "bad end")]
    fn panic_with_str() {
        run_test(|| panic!("bad end"))
    }

    #[test]
    #[should_panic(expected = "o! i am slain")]
    fn panic_formatted_argument() {
        run_test(|| panic!("o! i am {verb}", verb = "slain"));
    }

    #[test]
    #[should_panic(expected = "20130612")]
    fn return_err() {
        run_test(|| -> Result<(), i64> { Err(20130612) }).unwrap()
    }
}
