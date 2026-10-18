//! Port of UE1's `Sort` template (`HarryPotter2/Unreal/Core/Inc/UnTemplate.h:1727`).
//!
//! This is NOT a stable sort. The final order of elements that compare equal
//! depends on this exact hybrid quicksort plus bubble-sort swap sequence, so it
//! is ported line for line rather than replaced with `slice::sort_by`.
//!
//! `compare(a, b)` follows the UE1 tri-state convention used by the callers:
//! a result `> 0` means `a` ranks above `b`; anything else means it does not.

/// Sorts `items` in place with the UE1 `Sort` algorithm.
///
/// Indices are kept as `isize` so the pointer arithmetic of the original
/// (`Inner.Max-1-Current.Min`, `--Inner.Max`) maps directly.
pub fn ue_sort<T>(items: &mut [T], mut compare: impl FnMut(&T, &T) -> i32) {
    let num = items.len() as isize;
    if num < 2 {
        return;
    }

    // Pending (min, max) inclusive ranges. UE1 reserves 32 entries.
    let mut stack = [(0isize, 0isize); 32];
    stack[0] = (0, num - 1);
    let mut top: isize = 0;

    while top >= 0 {
        let (mut cur_min, mut cur_max) = stack[top as usize];
        'current: loop {
            let count = cur_max - cur_min + 1;
            if count <= 8 {
                // Simple bubble-sort.
                while cur_max > cur_min {
                    let mut max = cur_min;
                    let mut item = cur_min + 1;
                    while item <= cur_max {
                        if compare(&items[item as usize], &items[max as usize]) > 0 {
                            max = item;
                        }
                        item += 1;
                    }
                    items.swap(max as usize, cur_max as usize);
                    cur_max -= 1;
                }
            } else {
                // Grab the middle element so presorted input does not hit worst-case behaviour.
                items.swap((cur_min + count / 2) as usize, cur_min as usize);

                // Partition into items <= pivot (left) and items > pivot (right).
                let mut inner_min = cur_min;
                let mut inner_max = cur_max + 1;
                loop {
                    loop {
                        inner_min += 1;
                        if !(inner_min <= cur_max
                            && compare(&items[inner_min as usize], &items[cur_min as usize]) <= 0)
                        {
                            break;
                        }
                    }
                    loop {
                        inner_max -= 1;
                        if !(inner_max > cur_min
                            && compare(&items[inner_max as usize], &items[cur_min as usize]) >= 0)
                        {
                            break;
                        }
                    }
                    if inner_min > inner_max {
                        break;
                    }
                    items.swap(inner_min as usize, inner_max as usize);
                }
                items.swap(cur_min as usize, inner_max as usize);

                // Save the big half for later and continue with the small half.
                if inner_max - 1 - cur_min >= cur_max - inner_min {
                    if cur_min + 1 < inner_max {
                        stack[top as usize] = (cur_min, inner_max - 1);
                        top += 1;
                    }
                    if cur_max > inner_min {
                        cur_min = inner_min;
                        continue 'current;
                    }
                } else {
                    if cur_max > inner_min {
                        stack[top as usize] = (inner_min, cur_max);
                        top += 1;
                    }
                    if cur_min + 1 < inner_max {
                        cur_max = inner_max - 1;
                        continue 'current;
                    }
                }
            }
            break;
        }
        top -= 1;
    }
}
