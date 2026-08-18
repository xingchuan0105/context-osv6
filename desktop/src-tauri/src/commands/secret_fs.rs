//! Restrict secret files to the current user after write.

use std::path::Path;

pub fn restrict_secret_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        restrict_secret_file_windows(path);
    }
}

/// Owner-only DACL (`D:P(A;;FA;;;OW)`). AppData is already per-user; this
/// stops other local accounts from reading the file if the ACL is inherited
/// more loosely.
#[cfg(windows)]
fn restrict_secret_file_windows(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PACL, PSECURITY_DESCRIPTOR,
        PROTECTED_DACL_SECURITY_INFORMATION,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)\0".encode_utf16().collect();
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        ) == 0
        {
            return;
        }
        let mut present = 0i32;
        let mut defaulted = 0i32;
        let mut dacl: PACL = std::ptr::null_mut();
        if GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted) != 0
            && present != 0
        {
            let mut name = wide;
            let _ = SetNamedSecurityInfoW(
                name.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null_mut(),
            );
        }
        let _ = LocalFree(sd as _);
    }
}
