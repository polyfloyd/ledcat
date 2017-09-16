#[derive(Copy, Clone, Debug)]
pub enum Dimensions {
    One(usize),
    Two(usize, usize),
}

impl Dimensions {
    pub fn size(&self) -> usize {
        match *self {
            Dimensions::One(size) => size,
            Dimensions::Two(w, h) => w * h,
        }
    }
}


pub enum Axis {
    X,
    Y,
}


pub trait Transposition {
    fn transpose(&self, index: usize) -> usize;
}

impl Transposition for Vec<Box<Transposition>> {
    fn transpose(&self, index: usize) -> usize {
        self.iter().fold(index, |index, tr| tr.transpose(index))
    }
}


pub struct Reverse {
    pub length: usize,
}

impl Transposition for Reverse {
    fn transpose(&self, index: usize) -> usize {
        assert!(index < self.length);
        self.length - index - 1
    }
}


pub struct Mirror {
    pub width: usize,
    pub height: usize,
    pub axis: Axis,
}

impl Transposition for Mirror {
    fn transpose(&self, index: usize) -> usize {
        assert!(index < self.width * self.height);
        let x = index % self.width;
        let y = index / self.width;
        match self.axis {
            Axis::X => self.width * y + (self.width - x - 1),
            Axis::Y => self.width * (self.height - y - 1) + x,
        }
    }
}


pub struct Zigzag {
    pub width: usize,
    pub height: usize,
    pub major_axis: Axis,
}

impl Transposition for Zigzag {
    fn transpose(&self, index: usize) -> usize {
        assert!(index <= self.width * self.height);
        match self.major_axis {
            Axis::X => {
                let x = index % self.width;
                let y = index / self.width;
                if x % 2 == 0 {
                    x * self.height + y
                } else {
                    x * self.height + self.height - y - 1
                }
            }
            Axis::Y => {
                let y = index / self.width;
                let x = if y % 2 == 0 {
                    index % self.width
                } else {
                    self.width - index % self.width - 1
                };
                y * self.width + x
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::*;
    use super::*;

    fn transpose_all<T, I>(trans: T, input: I) -> Vec<usize>
        where T: Transposition,
              I: iter::Iterator<Item = usize> {
        input.map(|index| trans.transpose(index)).collect()
    }

    #[test]
    fn dimensions() {
        assert_eq!(42, Dimensions::One(42).size());
        assert_eq!(80, Dimensions::Two(4, 20).size());
    }

    #[test]
    fn transposition_list() {
        let tr: Vec<Box<Transposition>> = vec![
            Box::from(Zigzag {
                width: 4,
                height: 3,
                major_axis: Axis::Y,
            }),
            Box::from(Reverse { length: 4 * 3 }),
        ];
        assert_eq!(vec![11, 10, 9, 8, 4, 5, 6, 7, 3, 2, 1, 0],
                   transpose_all(tr, 0..12));
    }

    #[test]
    fn reverse() {
        assert_eq!(vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
                   transpose_all(Reverse { length: 10 }, 0..10));
    }

    #[test]
    fn mirror_x() {
        let m = Mirror {
            width: 3,
            height: 3,
            axis: Axis::X,
        };
        assert_eq!(vec![2, 1, 0, 5, 4, 3, 8, 7, 6], transpose_all(m, 0..9));
    }

    #[test]
    fn mirror_y() {
        let m = Mirror {
            width: 3,
            height: 3,
            axis: Axis::Y,
        };
        assert_eq!(vec![6, 7, 8, 3, 4, 5, 0, 1, 2], transpose_all(m, 0..9));
    }

    #[test]
    fn zigzag_x() {
        let zz = Zigzag {
            width: 3,
            height: 3,
            major_axis: Axis::X,
        };
        assert_eq!(vec![0, 5, 6, 1, 4, 7, 2, 3, 8], transpose_all(zz, 0..9));
        let zz = Zigzag {
            width: 4,
            height: 3,
            major_axis: Axis::X,
        };
        assert_eq!(vec![0, 5, 6, 11, 1, 4, 7, 10, 2, 3, 8, 9],
                   transpose_all(zz, 0..12));
    }

    #[test]
    fn zigzag_y() {
        let zz = Zigzag {
            width: 3,
            height: 3,
            major_axis: Axis::Y,
        };
        assert_eq!(vec![0, 1, 2, 5, 4, 3, 6, 7, 8], transpose_all(zz, 0..9));
        let zz = Zigzag {
            width: 4,
            height: 3,
            major_axis: Axis::Y,
        };
        assert_eq!(vec![0, 1, 2, 3, 7, 6, 5, 4, 8, 9, 10, 11],
                   transpose_all(zz, 0..12));
    }
}
