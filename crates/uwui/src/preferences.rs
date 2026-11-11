//! What the user chose over every look, which the interface applies to what it styles: the size of
//! text and less motion (`docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md`).
//!
//! A program gives them with [`Ui::set_preferences`](crate::tree::Ui::set_preferences), from its
//! own settings or from the system's. Density and more contrast are not here: they choose the
//! look's themes, which reach what code writes by the themes' names only, so that a length or a
//! colour that code writes as a number stays the author's.

/// What the user chose over every look: the size of text, and whether transitions end at once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preferences {
    /// How large each text is drawn, against the size that its style gives.
    pub text_size: TextSize,
    /// Whether the user asked for less motion: transitions end at once, those under way included,
    /// as `prefers-reduced-motion` asks of a web page.
    pub reduced_motion: bool,
}

/// The size of text that the user chose, a curve from the size that a style gives a text to the
/// size it is drawn at: larger text may grow less, as on Android 14.
///
/// It holds *stops*, each a size and the size it becomes, sorted by size, the sizes they become
/// never falling. Between two stops a size is interpolated linearly; below the first, from zero to
/// it, so at the first stop's ratio; beyond the last, at the last stop's ratio. The default is the
/// curve of the factor 1, which changes nothing.
///
/// ```
/// use uwui::preferences::TextSize;
///
/// let twice = TextSize::factor(2.0).expect("2 is within 0.5 and 3");
/// assert_eq!(twice.scale(13.0), 26.0);
///
/// // Small text doubles; text of 30 grows by a quarter.
/// let curve = TextSize::curve(&[(8.0, 16.0), (30.0, 37.5)]).expect("sorted, within its ratios");
/// assert_eq!(curve.scale(4.0), 8.0);
/// assert_eq!(curve.scale(19.0), 26.75);
/// assert_eq!(curve.scale(60.0), 75.0);
///
/// assert!(TextSize::factor(4.0).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct TextSize {
    /// The stops, sorted by size, never empty; a linear curve holds one.
    stops: Vec<(f32, f32)>,
}

/// The least and the most that the size of text multiplies a size by, GNOME's range: half, and
/// three times.
const RATIOS: std::ops::RangeInclusive<f32> = 0.5..=3.0;

/// Why a size of text was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextSizeError {
    /// A curve with no stop.
    #[error("a curve of the size of text needs one stop at least")]
    Empty,
    /// A factor, a size or the size it becomes that is not a finite number above zero.
    #[error("a size of text is a finite number above zero")]
    NotPositive,
    /// A stop or a factor that multiplies a size by less than a half or more than three.
    #[error("a size of text multiplies a size by 0.5 to 3")]
    OutOfRange,
    /// Stops whose sizes do not rise, or whose sizes become smaller as they rise.
    #[error("a curve's stops rise by their sizes, and the sizes they become never fall")]
    OutOfOrder,
}

impl TextSize {
    /// The curve that multiplies every size by `factor`, as GNOME's setting does.
    ///
    /// # Errors
    ///
    /// [`TextSizeError::NotPositive`] for a factor that is not a finite number above zero;
    /// [`TextSizeError::OutOfRange`] for one below 0.5 or above 3.
    pub fn factor(factor: f32) -> Result<Self, TextSizeError> {
        Self::curve(&[(1.0, factor)])
    }

    /// The curve through `stops`, each a size of text and the size it becomes, in logical pixels.
    ///
    /// # Errors
    ///
    /// [`TextSizeError::Empty`] for no stop; [`TextSizeError::NotPositive`] for a size that is not
    /// a finite number above zero; [`TextSizeError::OutOfRange`] for a stop that multiplies its
    /// size by less than 0.5 or more than 3; [`TextSizeError::OutOfOrder`] for sizes that do not
    /// rise, or sizes they become that fall.
    pub fn curve(stops: &[(f32, f32)]) -> Result<Self, TextSizeError> {
        if stops.is_empty() {
            return Err(TextSizeError::Empty);
        }
        for &(size, becomes) in stops {
            let positive = |value: f32| value.is_finite() && value > 0.0;
            if !positive(size) || !positive(becomes) {
                return Err(TextSizeError::NotPositive);
            }
            if !RATIOS.contains(&(becomes / size)) {
                return Err(TextSizeError::OutOfRange);
            }
        }
        if stops
            .windows(2)
            .any(|pair| pair[1].0 <= pair[0].0 || pair[1].1 < pair[0].1)
        {
            return Err(TextSizeError::OutOfOrder);
        }
        Ok(Self {
            stops: stops.to_vec(),
        })
    }

    /// The size that a text of `size` logical pixels is drawn at; `size` itself where it is not a
    /// finite number above zero.
    #[must_use]
    pub fn scale(&self, size: f32) -> f32 {
        if !(size.is_finite() && size > 0.0) {
            return size;
        }
        let first = self.stops[0];
        let last = self.stops[self.stops.len() - 1];
        if size <= first.0 {
            return size * (first.1 / first.0);
        }
        if size >= last.0 {
            return size * (last.1 / last.0);
        }
        let after = self.stops.partition_point(|&(stop, _)| stop <= size);
        let ((low, low_becomes), (high, high_becomes)) = (self.stops[after - 1], self.stops[after]);
        low_becomes + (high_becomes - low_becomes) * ((size - low) / (high - low))
    }

    /// What the curve multiplies a text of `size` by: 1 where `size` is not a finite number above
    /// zero.
    #[must_use]
    pub fn ratio(&self, size: f32) -> f32 {
        if size.is_finite() && size > 0.0 {
            self.scale(size) / size
        } else {
            1.0
        }
    }

    /// Whether it changes no size: the curve of the factor 1.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.stops.iter().all(|&(size, becomes)| size == becomes)
    }
}

impl Default for TextSize {
    fn default() -> Self {
        Self {
            stops: vec![(1.0, 1.0)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TextSize, TextSizeError};

    #[test]
    fn a_factor_multiplies_every_size() {
        let size = TextSize::factor(1.5).expect("within its range");
        for text in [1.0, 11.5, 13.0, 100.0] {
            assert_eq!(size.scale(text), text * 1.5);
        }
        assert!(TextSize::default().is_identity());
        assert!(!size.is_identity());
        assert_eq!(TextSize::default().scale(13.0), 13.0);
    }

    #[test]
    fn a_curve_interpolates_its_stops_and_keeps_its_ends_ratios() {
        // Android 14's table at 200 %, three of its stops.
        let size = TextSize::curve(&[(8.0, 16.0), (14.0, 26.0), (100.0, 100.0)]).expect("valid");
        assert_eq!(size.scale(8.0), 16.0);
        assert_eq!(
            size.scale(11.0),
            21.0,
            "halfway between the first two stops"
        );
        assert_eq!(size.scale(4.0), 8.0, "below the first stop, at its ratio");
        assert_eq!(size.scale(200.0), 200.0, "beyond the last, at its ratio");
        assert_eq!(size.scale(14.0), 26.0);
    }

    #[test]
    fn what_a_size_cannot_be_is_refused() {
        assert_eq!(TextSize::curve(&[]), Err(TextSizeError::Empty));
        for factor in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(TextSize::factor(factor), Err(TextSizeError::NotPositive));
        }
        assert_eq!(TextSize::factor(0.49), Err(TextSizeError::OutOfRange));
        assert_eq!(TextSize::factor(3.01), Err(TextSizeError::OutOfRange));
        assert!(TextSize::factor(0.5).is_ok() && TextSize::factor(3.0).is_ok());
        assert_eq!(
            TextSize::curve(&[(14.0, 26.0), (8.0, 16.0)]),
            Err(TextSizeError::OutOfOrder),
            "sizes that fall"
        );
        assert_eq!(
            TextSize::curve(&[(8.0, 16.0), (8.0, 16.0)]),
            Err(TextSizeError::OutOfOrder),
            "a size given twice"
        );
        assert_eq!(
            TextSize::curve(&[(8.0, 16.0), (10.0, 15.0)]),
            Err(TextSizeError::OutOfOrder),
            "sizes that become smaller"
        );
    }

    #[test]
    fn a_size_that_is_no_size_is_kept() {
        let size = TextSize::factor(2.0).expect("within its range");
        assert_eq!(size.scale(0.0), 0.0);
        assert!(size.scale(f32::NAN).is_nan());
        assert_eq!(size.ratio(-3.0), 1.0);
    }
}
