//! Internal declarations for semantic values with canonical HCI wire forms.

/// Declare a fieldless HCI enum and its exact-width wire encoding.
///
/// The generated decoder rejects values that are not one of the declared
/// discriminants, so the enum remains a closed protocol value in both
/// directions.
macro_rules! hci_enum {
    (
        $(#[$enum_attr:meta])*
        $vis:vis enum $name:ident : $repr:ty => $len:literal {
            $(
                $(#[$variant_attr:meta])*
                $variant:ident = $value:expr,
            )+
        }
    ) => {
        $(#[$enum_attr])*
        #[repr($repr)]
        $vis enum $name {
            $(
                $(#[$variant_attr])*
                $variant = $value,
            )+
        }

        impl crate::vendor::command::HciEncodeField<$len> for $name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                let value: $repr = match self {
                    $(Self::$variant => $value,)+
                };
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field(
                    &value,
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                let value: $repr = match self {
                    $(Self::$variant => $value,)+
                };
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field_async(
                    &value,
                    writer,
                )
                .await
            }
        }

        impl crate::vendor::command::HciDecodeField<$len> for $name {
            fn from_hci_field(
                bytes: &[u8; $len],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                let value =
                    <$repr as crate::vendor::command::HciDecodeField<$len>>::from_hci_field(bytes)?;
                $(
                    if value == $value {
                        return Ok(Self::$variant);
                    }
                )+
                Err(bt_hci::FromHciBytesError::InvalidValue)
            }
        }
    };
}

/// Declare HCI bitflags once while retaining `defmt`'s compact formatter.
///
/// The ordinary backend receives explicit standard derives because bitflags 2
/// no longer adds them automatically. The `defmt` backend wraps bitflags 1,
/// which already supplies those implementations.
macro_rules! hci_bitflags {
    (
        $(#[$($struct_attr:tt)*])*
        $vis:vis struct $name:ident : $repr:ty => $len:literal {
            $(
                $(#[$($flag_attr:tt)*])*
                const $flag:ident = $value:expr;
            )+
        }
    ) => {
        #[cfg(not(feature = "defmt"))]
        bitflags::bitflags! {
            $(#[$($struct_attr)*])*
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            $vis struct $name: $repr {
                $(
                    $(#[$($flag_attr)*])*
                    const $flag = $value;
                )+
            }
        }

        #[cfg(feature = "defmt")]
        defmt::bitflags! {
            $(#[$($struct_attr)*])*
            $vis struct $name: $repr {
                $(
                    $(#[$($flag_attr)*])*
                    const $flag = $value;
                )+
            }
        }

        impl crate::vendor::command::HciEncodeField<$len> for $name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field(
                    &self.bits(),
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field_async(
                    &self.bits(),
                    writer,
                )
                .await
            }
        }

        impl crate::vendor::command::HciDecodeField<$len> for $name {
            fn from_hci_field(
                bytes: &[u8; $len],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                let bits =
                    <$repr as crate::vendor::command::HciDecodeField<$len>>::from_hci_field(bytes)?;
                Self::from_bits(bits).ok_or(bt_hci::FromHciBytesError::InvalidValue)
            }
        }
    };
}
