use std::borrow::Cow;

use crate::type_ref::{Constness, ExternDir, StrEnc, StrType, TypeRef, TypeRefTypeHint};
use crate::writer::rust_native::type_ref::{Lifetime, TypeRefExt};

use super::{rust_arg_func_decl, rust_self_func_decl, RenderLaneTrait};

pub struct InStringRenderLane<'tu, 'ge> {
	str_type: StrType,
	non_canonical: TypeRef<'tu, 'ge>,
}

impl<'tu, 'ge> InStringRenderLane<'tu, 'ge> {
	pub fn from_str_type_non_canonical(str_type: StrType, non_canonical: TypeRef<'tu, 'ge>) -> Self {
		Self { str_type, non_canonical }
	}
}

impl RenderLaneTrait for InStringRenderLane<'_, '_> {
	fn rust_self_func_decl(&self, lifetime: Lifetime) -> Cow<'static, str> {
		rust_self_func_decl(Constness::Const, lifetime)
	}

	fn rust_arg_func_decl(&self, name: &str, _lifetime: Lifetime) -> String {
		let typ = if self.str_type.is_binary() {
			"&[u8]"
		} else {
			"&str"
		};
		rust_arg_func_decl(name, Constness::Const, typ)
	}

	fn rust_arg_pre_call(&self, name: &str, is_function_infallible: bool) -> String {
		if is_function_infallible {
			format!("extern_container_arg!(nofail {name})")
		} else {
			format!("extern_container_arg!({name})")
		}
	}

	fn rust_arg_func_call(&self, name: &str) -> String {
		format!("{name}.opencv_as_extern()")
	}

	fn rust_extern_arg_func_decl(&self, name: &str) -> String {
		rust_arg_func_decl(name, Constness::Const, &self.non_canonical.rust_extern(ExternDir::ToCpp))
	}

	fn cpp_arg_func_decl(&self, name: &str) -> String {
		format!("const char* {name}")
	}

	fn cpp_arg_func_call(&self, name: &str) -> String {
		match self.str_type {
			StrType::StdString(_) => format!("std::string({name})"),
			StrType::CvString(_) => format!("cv::String({name})"),
			StrType::CharPtr(_) => name.to_string(),
		}
	}
}

pub struct OutStringRenderLane<'tu, 'ge> {
	str_type: StrType,
	canonical: TypeRef<'tu, 'ge>,
}

impl<'tu, 'ge> OutStringRenderLane<'tu, 'ge> {
	pub fn from_str_type_canonical(str_type: StrType, canonical: TypeRef<'tu, 'ge>) -> Self {
		Self { str_type, canonical }
	}
}

impl RenderLaneTrait for OutStringRenderLane<'_, '_> {
	fn rust_self_func_decl(&self, lifetime: Lifetime) -> Cow<'static, str> {
		rust_self_func_decl(Constness::Mut, lifetime)
	}

	fn rust_arg_func_decl(&self, name: &str, _lifetime: Lifetime) -> String {
		let typ = if self.str_type.is_binary() {
			"&mut Vec<u8>"
		} else {
			"&mut String"
		};
		rust_arg_func_decl(name, Constness::Const, typ)
	}

	fn rust_arg_pre_call(&self, name: &str, _is_function_infallible: bool) -> String {
		format!("string_arg_output_send!(via {name}_via)")
	}

	fn rust_arg_func_call(&self, name: &str) -> String {
		format!("&mut {name}_via")
	}

	fn rust_arg_post_success_call(&self, name: &str) -> String {
		format!("string_arg_output_receive!({name}_via => {name})")
	}

	fn rust_extern_arg_func_decl(&self, name: &str) -> String {
		rust_arg_func_decl(name, Constness::Const, &self.canonical.rust_extern(ExternDir::ToCpp))
	}

	fn cpp_arg_func_decl(&self, name: &str) -> String {
		format!("void** {name}")
	}

	fn cpp_arg_pre_call(&self, name: &str) -> String {
		match self.str_type {
			StrType::StdString(_) => format!("std::string {name}_out"),
			StrType::CvString(_) => format!("cv::String {name}_out"),
			StrType::CharPtr(str_enc) => {
				let len = if matches!(str_enc, StrEnc::Binary) {
					if let TypeRefTypeHint::StringAsBytes(Some(len_arg_name)) = self.canonical.type_hint() {
						len_arg_name.as_str()
					} else {
						"1024"
					}
				} else {
					"1024"
				};
				format!("std::unique_ptr<char[]> {name}_out = std::make_unique<char[]>({len})")
			}
		}
	}

	fn cpp_arg_func_call(&self, name: &str) -> String {
		match self.str_type {
			StrType::StdString(_) | StrType::CvString(_) => {
				let ptr = if self.canonical.kind().as_pointer().is_some() {
					"&"
				} else {
					""
				};
				format!("{ptr}{name}_out")
			}
			StrType::CharPtr(_) => format!("{name}_out.get()"),
		}
	}

	fn cpp_arg_post_call(&self, name: &str) -> String {
		match self.str_type {
			StrType::StdString(StrEnc::Text) | StrType::CvString(StrEnc::Text) => {
				format!("*{name} = ocvrs_create_string({name}_out.c_str())")
			}
			StrType::StdString(StrEnc::Binary) => {
				format!("*{name} = ocvrs_create_byte_string({name}_out.data(), {name}_out.size())")
			}
			StrType::CvString(StrEnc::Binary) => {
				format!("*{name} = ocvrs_create_byte_string({name}_out.begin(), {name}_out.size())")
			}
			StrType::CharPtr(StrEnc::Text) => {
				format!("*{name} = ocvrs_create_string({name}_out.get())")
			}
			StrType::CharPtr(StrEnc::Binary) => {
				if let TypeRefTypeHint::StringAsBytes(Some(len_arg_name)) = self.canonical.type_hint() {
					format!("*{name} = ocvrs_create_byte_string({name}_out.get(), {len_arg_name})")
				} else {
					panic!("Output argument of type `char*` with binary encoding must have `len` argument specified")
				}
			}
		}
	}
}
