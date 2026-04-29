macro_rules! impl_params {
    ($method:ident, $param_type:ident, $opcode:path) => {
        async fn $method(&mut self, params: &$param_type) {
            crate::write_fixed_with_opcode(self, $opcode, $param_type::LENGTH, |buf| {
                params.copy_into_slice(buf);
            })
            .await
        }
    };
}

macro_rules! impl_value_params {
    ($method:ident, $param_type:ident, $opcode:path) => {
        async fn $method(&mut self, params: $param_type) {
            crate::write_fixed_with_opcode(self, $opcode, $param_type::LENGTH, |buf| {
                params.copy_into_slice(buf);
            })
            .await
        }
    };
}

macro_rules! impl_validate_params {
    ($method:ident, $param_type:ident, $opcode:path) => {
        async fn $method(&mut self, params: &$param_type) -> Result<(), Error> {
            params.validate()?;

            crate::write_fixed_with_opcode(self, $opcode, $param_type::LENGTH, |buf| {
                params.copy_into_slice(buf);
            })
            .await;

            Ok(())
        }
    };
}

macro_rules! impl_variable_length_params {
    ($method:ident, $param_type:ident, $opcode:path) => {
        async fn $method(&mut self, params: &$param_type) {
            crate::write_fixed_with_opcode(self, $opcode, $param_type::MAX_LENGTH, |buf| {
                params.copy_into_slice(buf);
            })
            .await;
        }
    };
    ($method:ident<$($genlife:lifetime),*>, $param_type:ident<$($lifetime:lifetime),*>, $opcode:path) => {
        async fn $method<$($genlife),*>(
            &mut self,
            params: &$param_type<$($lifetime),*>
        ) {
            crate::write_fixed_with_opcode(self, $opcode, $param_type::MAX_LENGTH, |buf| {
                params.copy_into_slice(buf);
            })
            .await;
        }
    };
}

macro_rules! impl_validate_variable_length_params {
    ($method:ident, $param_type:ident, $opcode:path) => {
        async fn $method(&mut self, params: &$param_type) -> Result<(), Error> {
            params.validate()?;

            crate::write_with_opcode(self, $opcode, |buf| {
                params.copy_into_slice(&mut buf[..$param_type::MAX_LENGTH])
            })
            .await;

            Ok(())
        }
    };
    ($method:ident<$($genlife:lifetime),*>, $param_type:ident<$($lifetime:lifetime),*>, $opcode:path) => {
        async fn $method<$($genlife),*>(
            &mut self,
            params: &$param_type<$($lifetime),*>
        ) -> Result<(), Error> {
            params.validate()?;

            crate::write_with_opcode(self, $opcode, |buf| {
                params.copy_into_slice(&mut buf[..$param_type::MAX_LENGTH])
            })
            .await;

            Ok(())
        }
    };
}

pub mod gap;
pub mod gatt;
pub mod hal;
pub mod l2cap;
