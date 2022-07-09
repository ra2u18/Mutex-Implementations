use std::cell::UnsafeCell;
use std::sync::atomic::{ AtomicBool, Ordering };

const LOCKED: bool = true;
const UNLOCKED: bool = false;

pub struct Mutex<T> {
    locked: AtomicBool,
    v: UnsafeCell<T>
}

unsafe impl<T> Sync for Mutex<T> where T: Send {}

impl<T> Mutex<T> {
    pub fn new(t: T) -> Self {
        Self {
            locked: AtomicBool::new(UNLOCKED),
            v: UnsafeCell::new(t)
        }
    } 

    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // x86: CAS
        // ARM: LDREX STREX
        //      - compare_exchange: impl using a loop of LDREX and STREx
        //      - compare_exchange_weak: LDREX STREX
        while self.locked.compare_exchange_weak(
          UNLOCKED, 
          LOCKED, 
          Ordering::Acquire, 
          Ordering::Relaxed).is_err() 
        {
            // Mesi protocol: stay in S when locked
            while self.locked.load(Ordering::Relaxed) == LOCKED {}
        }

        let res = f(unsafe { &mut *self.v.get() });
        self.locked.store(UNLOCKED, Ordering::Release);

        res
    }
}

fn main() {
    let l: &'static _ = Box::leak(Box::new(Mutex::new(0)));
    let handles: Vec<_> = (0..10)
        .map(|_| {
            std::thread::spawn(move || {
                for _ in 0..100 {
                    l.with_lock(|v| {
                        *v += 1;
                    });
                }
            })
        }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(l.with_lock(|v| *v), 10 * 100);
}

#[test]
fn too_relaxed() {
    use std::sync::atomic::{ AtomicUsize };

    let x: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));
    let y: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));

    let t1 = std::thread::spawn(move || {
        let r1 = y.load(Ordering::Relaxed);
        x.store(r1, Ordering::Relaxed);
        r1
    });

    let t2 = std::thread::spawn(move || {
        let r2 = x.load(Ordering::Relaxed);
        y.store(42, Ordering::Relaxed);
        r2
    });

    let r1 = t1.join().unwrap();
    let r2 = t2.join().unwrap();
}