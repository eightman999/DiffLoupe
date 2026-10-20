//! Myers 差分(Swift `Collection.difference(from:)` 相当の
//! 削除/挿入オフセット集合を返す)。双方向線形空間 Myers 法。

/// side(base→side)の差分から、base側の削除オフセットとside側の挿入オフセットを返す。
pub fn diff_offsets<T: PartialEq>(base: &[T], side: &[T]) -> (Vec<bool>, Vec<bool>) {
    let mut removed = vec![false; base.len()];
    let mut inserted = vec![false; side.len()];
    for (kind, i) in myers_changes(base, side) {
        match kind {
            Change::Remove => removed[i] = true,
            Change::Insert => inserted[i] = true,
        }
    }
    (removed, inserted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Remove,
    Insert,
}

/// Myers O(ND) 全トレース法。削除は base インデックス、挿入は side インデックス。
/// Swift の差分と同じく「先に挿入が来る貪欲経路」に近い解を選ぶ。
fn myers_changes<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(Change, usize)> {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    if max == 0 {
        return vec![];
    }
    let width = 2 * max + 1;
    let offset = max as isize;
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut v = vec![0isize; width];
    let mut found_d: Option<usize> = None;
    'outer: for d in 0..=(max as isize) {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d
                || (k != d && v[(offset + k - 1) as usize] < v[(offset + k + 1) as usize])
            {
                v[(offset + k + 1) as usize]
            } else {
                v[(offset + k - 1) as usize] + 1
            };
            let mut y = x - k;
            while (x as usize) < n && (y as usize) < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(offset + k) as usize] = x;
            if x as usize >= n && y as usize >= m {
                found_d = Some(d as usize);
                break 'outer;
            }
            k += 2;
        }
    }
    let _ = v;
    // バックトラック
    let mut changes: Vec<(Change, usize)> = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;
    // trace[d] は深さ d の処理前の V(=V_{d-1})。深さ d の手を
    // 遡るには trace[d] を見る → d は found_d から 1 まで降順。
    for d in (1..=found_d.unwrap()).rev() {
        let d = d as isize;
        let v = &trace[d as usize];
        let k = x - y;
        let prev_k =
            if k == -d || (k != d && v[(offset + k - 1) as usize] < v[(offset + k + 1) as usize]) {
                k + 1
            } else {
                k - 1
            };
        let prev_x = v[(offset + prev_k) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
        }
        if x == prev_x {
            changes.push((Change::Insert, prev_y as usize));
        } else {
            changes.push((Change::Remove, prev_x as usize));
        }
        x = prev_x;
        y = prev_y;
    }
    changes.reverse();
    changes
}
