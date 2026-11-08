//! Shared engine for `UnTemplate.h`'s `template<class T> void Sort(T* First, INT Num)`
//! (`HarryPotter2/Unreal/Core/Inc/UnTemplate.h:1726`): a non-stable quicksort with an
//! explicit 32-entry recursion stack, a middle-element pivot, and an 8-element
//! bubble-sort tail.
//!
//! The algorithm is expressed purely over indices via injected `compare`/`swap`
//! callbacks so it can drive a Rust slice directly or FFI-owned raw-pointer
//! storage (comparisons/swaps crossing back into C++) without copying elements
//! or requiring the caller to hand over a real `&mut [T]`.
//!
//! `on_iteration` runs once per algorithm step, exactly where the native
//! `checkSlow( Current.Max>=Current.Min )` sits at the `Loop:` label (both on
//! first entry to a popped stack range and on every `goto Loop` re-entry).
//! Callers that don't mirror that checked-build assertion pass `|_| Ok(())`.
pub fn quicksort<E>(
    count: isize,
    mut on_iteration: impl FnMut(bool) -> Result<(), E>,
    mut compare: impl FnMut(isize, isize) -> Result<i32, E>,
    mut swap: impl FnMut(isize, isize),
) -> Result<(), E> {
    if count < 2 {
        return Ok(());
    }
    let mut stack = [(0_isize, 0_isize); 32];
    stack[0] = (0, count - 1);
    let mut top: isize = 0;
    while top >= 0 {
        let (mut cmin, mut cmax) = stack[top as usize];
        loop {
            on_iteration(cmax >= cmin)?;
            let n = cmax - cmin + 1;
            if n <= 8 {
                // Use simple bubble-sort.
                while cmax > cmin {
                    let mut max = cmin;
                    let mut item = cmin + 1;
                    while item <= cmax {
                        if compare(item, max)? > 0 {
                            max = item;
                        }
                        item += 1;
                    }
                    swap(max, cmax);
                    cmax -= 1;
                }
                break;
            }
            // Grab middle element so sort doesn't exhibit worst-case behaviour with
            // presorted lists.
            swap(cmin + n / 2, cmin);

            // Divide list into two halves, one with items <=cmin, the other with
            // items >cmax.
            let mut imin = cmin;
            let mut imax = cmax + 1;
            loop {
                loop {
                    imin += 1;
                    if !(imin <= cmax && compare(imin, cmin)? <= 0) {
                        break;
                    }
                }
                loop {
                    imax -= 1;
                    if !(imax > cmin && compare(imax, cmin)? >= 0) {
                        break;
                    }
                }
                if imin > imax {
                    break;
                }
                swap(imin, imax);
            }
            swap(cmin, imax);

            // Save big half and recurse with small half.
            if imax - 1 - cmin >= cmax - imin {
                if cmin + 1 < imax {
                    stack[top as usize] = (cmin, imax - 1);
                    top += 1;
                }
                if cmax > imin {
                    cmin = imin;
                    continue;
                }
                break;
            } else {
                if cmax > imin {
                    stack[top as usize] = (imin, cmax);
                    top += 1;
                }
                if cmin + 1 < imax {
                    cmax = imax - 1;
                    continue;
                }
                break;
            }
        }
        top -= 1;
    }
    Ok(())
}
