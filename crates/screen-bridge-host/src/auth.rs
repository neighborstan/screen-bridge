//! RTSP Basic auth для production host.

use gstreamer_rtsp_server::gst;
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{
    RTSPAuth, RTSPMediaFactory, RTSPToken, RTSP_PERM_MEDIA_FACTORY_ACCESS,
    RTSP_PERM_MEDIA_FACTORY_CONSTRUCT, RTSP_TOKEN_MEDIA_FACTORY_ROLE,
};
use screen_bridge_core::config::SecurityConfig;

const AUTHENTICATED_ROLE: &str = "screen-bridge-viewer";

pub(crate) fn configure_auth(auth: &RTSPAuth, config: &SecurityConfig) {
    auth.set_supported_methods(gst_rtsp::RTSPAuthMethod::Basic);

    let token = build_viewer_token();
    let basic = RTSPAuth::make_basic(&config.auth_user, config.access_token.as_str());
    auth.add_basic(&basic, &token);
}

pub(crate) fn require_authentication(factory: &RTSPMediaFactory) {
    let role = build_viewer_role_structure();
    factory.add_role_from_structure(role.as_ref());
}

fn build_viewer_token() -> RTSPToken {
    RTSPToken::builder()
        .field_with_static(RTSP_TOKEN_MEDIA_FACTORY_ROLE, AUTHENTICATED_ROLE)
        .build()
}

fn build_viewer_role_structure() -> gst::Structure {
    gst::Structure::builder(AUTHENTICATED_ROLE)
        .field_with_static(RTSP_PERM_MEDIA_FACTORY_ACCESS, true)
        .field_with_static(RTSP_PERM_MEDIA_FACTORY_CONSTRUCT, true)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_token_should_have_media_factory_role() {
        // Given / When
        gstreamer::init().unwrap();
        let token = build_viewer_token();

        // Then
        assert_eq!(
            token
                .string(RTSP_TOKEN_MEDIA_FACTORY_ROLE.as_str())
                .as_deref(),
            Some(AUTHENTICATED_ROLE)
        );
    }

    #[test]
    fn viewer_role_should_allow_media_factory_access_and_construct() {
        // Given / When
        gstreamer::init().unwrap();
        let role = build_viewer_role_structure();

        // Then
        assert_eq!(role.name(), AUTHENTICATED_ROLE);
        assert!(role
            .get::<bool>(RTSP_PERM_MEDIA_FACTORY_ACCESS.as_str())
            .unwrap());
        assert!(role
            .get::<bool>(RTSP_PERM_MEDIA_FACTORY_CONSTRUCT.as_str())
            .unwrap());
    }

    #[test]
    fn basic_credentials_should_not_contain_plain_token() {
        // Given
        gstreamer::init().unwrap();
        let user = "viewer";
        let token = "valid-token-1234";

        // When
        let basic = RTSPAuth::make_basic(user, token);

        // Then
        assert!(!basic.is_empty());
        assert!(!basic.contains(user));
        assert!(!basic.contains(token));
    }
}
