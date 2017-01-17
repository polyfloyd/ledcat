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


pub trait Transposition {
    fn transpose(&self, index: usize) -> usize;
}

impl Transposition for Vec<Box<Transposition>> {

    fn transpose(&self, index: usize) -> usize {
        self.iter().fold(index, |index, tr| tr.transpose(index))
    }

}


pub struct Reverse {
    pub num_pixels: usize,
}

impl Transposition for Reverse {

    fn transpose(&self, index: usize) -> usize {
        assert!(index < self.num_pixels);
        self.num_pixels - index - 1
    }

}


pub enum Axis { X, Y }

pub struct Zigzag {
    pub width:      usize,
    pub height:     usize,
    pub major_axis: Axis,
}

impl Transposition for Zigzag {

    fn transpose(&self, index: usize) -> usize {
        assert!(index <= self.width * self.height);
        match self.major_axis {
            Axis::X => {
                let x = index / self.height;
                let y = if x % 2 == 0 {
                    index % self.height
                } else {
                    self.height - index % self.height - 1
                };
                y * self.width + x
            },
            Axis::Y => {
                let y = index / self.width;
                let x = if y % 2 == 0 {
                    index % self.width
                } else {
                    self.width - index % self.width - 1
                };
                y * self.width + x
            },
        }
    }

}
