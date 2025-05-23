//! Traits for arithmetic operations on elliptic curve field elements.

pub use core::ops::{Add, AddAssign, Mul, Neg, Shr, ShrAssign, Sub, SubAssign};
pub use crypto_bigint::Invert;

use crypto_bigint::Integer;
use subtle::{Choice, ConditionallySelectable, CtOption};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Perform a batched inversion on a sequence of field elements (i.e. base field elements or scalars)
/// at an amortized cost that should be practically as efficient as a single inversion.
pub trait BatchInvert<FieldElements: ?Sized>: Invert + Sized {
    /// The output of batch inversion. A container of field elements.
    type Output: AsRef<[Self]>;

    /// Invert a batch of field elements.
    fn batch_invert(
        field_elements: &FieldElements,
    ) -> CtOption<<Self as BatchInvert<FieldElements>>::Output>;
}

impl<const N: usize, T> BatchInvert<[T; N]> for T
where
    T: Invert<Output = CtOption<Self>>
        + Mul<Self, Output = Self>
        + Copy
        + Default
        + ConditionallySelectable,
{
    type Output = [Self; N];

    fn batch_invert(field_elements: &[Self; N]) -> CtOption<[Self; N]> {
        let mut field_elements_multiples = [Self::default(); N];
        let mut field_elements_multiples_inverses = [Self::default(); N];
        let mut field_elements_inverses = [Self::default(); N];

        let inversion_succeeded = invert_batch_internal(
            field_elements,
            &mut field_elements_multiples,
            &mut field_elements_multiples_inverses,
            &mut field_elements_inverses,
        );

        CtOption::new(field_elements_inverses, inversion_succeeded)
    }
}

#[cfg(feature = "alloc")]
impl<T> BatchInvert<[T]> for T
where
    T: Invert<Output = CtOption<Self>>
        + Mul<Self, Output = Self>
        + Copy
        + Default
        + ConditionallySelectable,
{
    type Output = Vec<Self>;

    fn batch_invert(field_elements: &[Self]) -> CtOption<Vec<Self>> {
        let mut field_elements_multiples: Vec<Self> = vec![Self::default(); field_elements.len()];
        let mut field_elements_multiples_inverses: Vec<Self> =
            vec![Self::default(); field_elements.len()];
        let mut field_elements_inverses: Vec<Self> = vec![Self::default(); field_elements.len()];

        let inversion_succeeded = invert_batch_internal(
            field_elements,
            field_elements_multiples.as_mut(),
            field_elements_multiples_inverses.as_mut(),
            field_elements_inverses.as_mut(),
        );

        CtOption::new(
            field_elements_inverses.into_iter().collect(),
            inversion_succeeded,
        )
    }
}

/// Implements "Montgomery's trick", a trick for computing many modular inverses at once.
///
/// "Montgomery's trick" works by reducing the problem of computing `n` inverses
/// to computing a single inversion, plus some storage and `O(n)` extra multiplications.
///
/// See: https://iacr.org/archive/pkc2004/29470042/29470042.pdf section 2.2.
fn invert_batch_internal<
    T: Invert<Output = CtOption<T>> + Mul<T, Output = T> + Default + ConditionallySelectable,
>(
    field_elements: &[T],
    field_elements_multiples: &mut [T],
    field_elements_multiples_inverses: &mut [T],
    field_elements_inverses: &mut [T],
) -> Choice {
    let batch_size = field_elements.len();
    if batch_size == 0
        || batch_size != field_elements_multiples.len()
        || batch_size != field_elements_multiples_inverses.len()
    {
        return Choice::from(0);
    }

    field_elements_multiples[0] = field_elements[0];
    for i in 1..batch_size {
        // $ a_n = a_{n-1}*x_n $
        field_elements_multiples[i] = field_elements_multiples[i - 1] * field_elements[i];
    }

    field_elements_multiples[batch_size - 1]
        .invert()
        .map(|multiple_of_inverses_of_all_field_elements| {
            field_elements_multiples_inverses[batch_size - 1] =
                multiple_of_inverses_of_all_field_elements;
            for i in (1..batch_size).rev() {
                // $ a_{n-1} = {a_n}^{-1}*x_n $
                field_elements_multiples_inverses[i - 1] =
                    field_elements_multiples_inverses[i] * field_elements[i];
            }

            field_elements_inverses[0] = field_elements_multiples_inverses[0];
            for i in 1..batch_size {
                // $ {x_n}^{-1} = a_{n}^{-1}*a_{n-1} $
                field_elements_inverses[i] =
                    field_elements_multiples_inverses[i] * field_elements_multiples[i - 1];
            }
        })
        .is_some()
}

/// Linear combination.
///
/// This trait enables optimized implementations of linear combinations (e.g. Shamir's Trick).
///
/// It's generic around `PointsAndScalars` to allow overlapping impls. For example, const generic
/// impls can use the input size to determine the size needed to store temporary variables.
pub trait LinearCombination<PointsAndScalars>: group::Curve
where
    PointsAndScalars: AsRef<[(Self, Self::Scalar)]> + ?Sized,
{
    /// Calculates `x1 * k1 + ... + xn * kn`.
    fn lincomb(points_and_scalars: &PointsAndScalars) -> Self {
        points_and_scalars
            .as_ref()
            .iter()
            .copied()
            .map(|(point, scalar)| point * scalar)
            .sum()
    }
}

/// Modular reduction.
pub trait Reduce<Uint: Integer>: Sized {
    /// Bytes used as input to [`Reduce::reduce_bytes`].
    type Bytes: AsRef<[u8]>;

    /// Perform a modular reduction, returning a field element.
    fn reduce(n: Uint) -> Self;

    /// Interpret the given bytes as an integer and perform a modular reduction.
    fn reduce_bytes(bytes: &Self::Bytes) -> Self;
}

/// Modular reduction to a non-zero output.
///
/// This trait is primarily intended for use by curve implementations such
/// as the `k256` and `p256` crates.
///
/// End users should use the [`Reduce`] impl on
/// [`NonZeroScalar`][`crate::NonZeroScalar`] instead.
pub trait ReduceNonZero<Uint: Integer>: Reduce<Uint> + Sized {
    /// Perform a modular reduction, returning a field element.
    fn reduce_nonzero(n: Uint) -> Self;

    /// Interpret the given bytes as an integer and perform a modular reduction
    /// to a non-zero output.
    fn reduce_nonzero_bytes(bytes: &Self::Bytes) -> Self;
}
