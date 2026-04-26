

pub fn convert__soc_projection_kind__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::soc_projection_kind::SocProjectionKind) -> crate::victron_controller::dashboard::soc_projection_kind::SocProjectionKind {
    from.to_string().parse().expect("enum parse")
}