macro_rules! match_helper {
    ([size] $expr:expr, $($name:tt,)*) => {
        match $expr {
            $(
                Self::$name(box_) => box_.size(),
            )*
            Self::Unknown((_, data)) => {
                let size = data.len() as u64 + 8;
                if size > u32::MAX as u64 {
                    size + 8
                } else {
                    size
                }
            }
        }
    };
    ([write] $expr:expr, $writer:expr, $($name:tt,)*) => {
        match $expr {
            $(
                Self::$name(box_) => box_.mux($writer)?,
            )*
            Self::Unknown((header, data)) => {
                let size = data.len() as u64 + 8;
                if size > u32::MAX as u64 {
                    $writer.write_u32::<byteorder::BigEndian>(1)?;
                } else {
                    $writer.write_u32::<byteorder::BigEndian>(size as u32)?;
                }
                $writer.write_all(&header.box_type)?;
                if size > u32::MAX as u64 {
                    $writer.write_u64::<byteorder::BigEndian>(size)?;
                }
                $writer.write_all(data)?;
            }
        }
    };
    ([parse] $expr:expr, $header:expr, $data:expr, $($name:tt,)*) => {
        match $expr {
            $(
                &$name::NAME => Ok(Self::$name(<$name>::demux($header, $data)?)),
            )*
            _ => Ok(Self::Unknown(($header, $data))),
        }
    };
}

macro_rules! as_fn {
    ($($type:tt,)*) => {
        $(
            paste! {
                #[allow(dead_code)]
                pub fn [<as_ $type:lower>](&self) -> Option<&$type> {
                    match self {
                        Self::$type(box_) => Some(box_),
                        _ => None,
                    }
                }
            }
        )*
    };
}

macro_rules! impl_from {
    ($($type:tt,)*) => {
        $(
            impl From<$type> for DynBox {
                fn from(box_: $type) -> Self {
                    Self::$type(box_)
                }
            }
        )*
    };
}

macro_rules! impl_box {
    ($($type:tt,)*) => {
        #[derive(Debug, Clone, PartialEq)]
        pub enum DynBox {
            $(
                $type($type),
            )*
            Unknown((BoxHeader, Bytes)),
        }

        impl DynBox {
            pub fn size(&self) -> u64 {
                match_helper!(
                    [size] self,
                    $($type,)*
                )
            }

            pub fn mux<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
                match_helper!(
                    [write] self, writer,
                    $($type,)*
                );

                Ok(())
            }

            pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
                let (header, data) = BoxHeader::demux(reader)?;

                match_helper!(
                    [parse] & header.box_type,
                    header,
                    data,
                    $($type,)*
                )
            }

            pub fn name(&self) -> &str {
                match self {
                    $(
                        Self::$type(_) => std::str::from_utf8(&$type::NAME).expect("invalid utf8"),
                    )*
                    Self::Unknown((header, _)) => std::str::from_utf8(&header.box_type).unwrap_or("unknown"),
                }
            }

            as_fn!(
                $($type,)*
            );
        }


        impl_from!(
            $($type,)*
        );
    };
}
