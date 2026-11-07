use super::{CmdResult, proxy_aware_coded_error};
use crate::{
    constants::timing,
    core::{
        CoreManager,
        manager::RunningMode,
        service::{SERVICE_MANAGER, ServiceStatus, request_runtime_provider_sync},
    },
};

/// Signals that the *elevation prompt* failed, so the installer never really ran.
///
/// The installer crate elevates with `pkexec` (falling back to `sudo`) and reports
/// only `elevated installer failed with <status>` when that fails. Every distinct
/// cause arrives looking the same — no polkit agent on a headless session, a
/// dismissed authentication dialog, a wrong password, a missing `pkexec` *and* no
/// usable `sudo`. None of them are the student's fault and all of them look
/// identical to us, so we do not pretend to tell them apart. What we *can* do is
/// stop reporting it as "the service failed to install", which sends someone to
/// look for a service problem that does not exist.
const ELEVATION_FAILED_CODE: &str = "SERVICE_ELEVATION_FAILED";

/// A student-facing sentence for a failed elevation prompt.
///
/// Names the two things that actually resolve it on a Linux desktop, because
/// either one works and we cannot tell which applies from here.
const ELEVATION_FAILED_MESSAGE: &str = "Locus asked for administrator permission to install its \
     service, but the request was not approved. Approve the permission prompt and try again; if no \
     prompt appeared, run Locus from a terminal so it can ask for a password there.";

/// Recognise the installer's elevation failure and describe it usefully.
///
/// Matches on the message the installer crate emits (`elevated installer failed
/// with ...`), because the failure crosses a process boundary as an exit status
/// and there is no typed error to downcast. That is a string match against
/// upstream wording, which is a coupling worth stating plainly: if the installer
/// changes its message this classifier stops firing and the operation degrades to
/// the generic code, which is the safe direction to fail in.
fn elevation_failure(error: &anyhow::Error) -> Option<super::CommandFailure> {
    let chain = format!("{error:#}");
    if !chain.contains("elevated installer failed") {
        return None;
    }
    Some(super::coded_error(ELEVATION_FAILED_CODE, ELEVATION_FAILED_MESSAGE))
}

/// The privileged helper that would have run is not in this installation.
///
/// Its own code because it is **not** an elevation problem and **not** an install
/// problem: nothing was attempted, so "approve the prompt and try again" and
/// "check your configuration" are both wrong things to tell a student. The helper
/// is a file inside the bundle, so either the installation is incomplete or
/// something removed it — and on Windows the bundle's helpers are prebuilt
/// binaries that need a system runtime beside them, which is the one prerequisite
/// this app has and does not carry.
const MISSING_HELPER_CODE: &str = "SERVICE_HELPER_MISSING";

/// A sentence naming the file and the route that repairs it.
///
/// No reinstall-the-VC++-runtime advice as a first move: on the machine that
/// reported this, reinstalling Locus from the official installer would have
/// replaced the helper and been the faster path. The runtime is named as the
/// *second* possibility because it is the one thing a missing file cannot explain.
fn missing_helper_message(tool: &str) -> std::string::String {
    format!(
        "Locus could not start its service installer: the file {tool} is missing from this \
         installation. Reinstall Locus from the installer you downloaded. If the same message \
         still names this file after a fresh install, the Microsoft Visual C++ Redistributable is \
         missing from this PC — install it and try again."
    )
}

/// Downcast first, then the runtime case, then fall through.
///
/// The downcast is the honest half: `MissingServiceTool` is raised by us and names
/// the exact path. The second arm matches on the constant attached in
/// `core::service`, which is a string match — but against a literal this repository
/// owns, not a dependency's prose, so a reworded upstream message cannot silently
/// move a student into the wrong branch.
fn missing_helper_failure(error: &anyhow::Error) -> Option<super::CommandFailure> {
    if let Some(missing) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<crate::core::service::MissingServiceTool>())
    {
        return Some(super::coded_error(
            MISSING_HELPER_CODE,
            missing_helper_message(&missing.tool),
        ));
    }

    let chain = format!("{error:#}");
    if chain.contains(crate::core::service::MISSING_RUNTIME_CONTEXT) {
        let runtime = crate::core::service::missing_runtime_sentence();
        let runtime = if runtime.is_empty() { "" } else { runtime };
        return Some(super::coded_error(
            MISSING_HELPER_CODE,
            format!(
                "Locus's service installer could not start because a system library it needs is \
                 missing from this PC.{runtime} Install the Microsoft Visual C++ Redistributable \
                 and try again; if it still fails, reinstall Locus."
            ),
        ));
    }

    None
}

/// Classifies a privileged-operation failure into the most specific code we have.
///
/// Ordered from most specific to least: a helper that never ran is a different
/// fault from an elevation that was refused, and both are different from a Service
/// that was configured and refused. Each has its own next step.
fn classify_privileged_failure(error: &anyhow::Error, error_code: &str) -> super::CommandFailure {
    missing_helper_failure(error)
        .or_else(|| elevation_failure(error))
        .unwrap_or_else(|| proxy_aware_coded_error(error, error_code))
}

async fn execute_service_operation_sync(status: ServiceStatus, error_code: &str) -> CmdResult {
    let manager = CoreManager::global();
    let _lifecycle = manager.lifecycle_lock.lock().await;
    if matches!(
        &status,
        ServiceStatus::ReinstallRequired | ServiceStatus::ForceReinstallRequired
    ) {
        manager
            .controlled_stop_core_inner()
            .await
            .map_err(|error| proxy_aware_coded_error(&error, error_code))?;
    }
    SERVICE_MANAGER
        .handle_service_status(status)
        .await
        .map_err(|error| classify_privileged_failure(&error, error_code))
}

#[tauri::command]
pub async fn install_service() -> CmdResult {
    execute_service_operation_sync(ServiceStatus::InstallRequired, "SERVICE_INSTALL_FAILED").await
}

/// Removes the Service and stops the Core.
///
/// A tunnel cannot outlive its Service any more, so this deliberately stops
/// rather than reaching for another way to run the engine: leaving the Core up
/// would report a tunnel whose route the product would then refuse to supervise.
#[tauri::command]
pub async fn uninstall_service() -> CmdResult {
    execute_service_operation_sync(ServiceStatus::UninstallRequired, "SERVICE_UNINSTALL_FAILED").await
}

#[tauri::command]
pub async fn reinstall_service() -> CmdResult {
    execute_service_operation_sync(ServiceStatus::ReinstallRequired, "SERVICE_REINSTALL_FAILED").await
}

#[tauri::command]
pub async fn repair_service() -> CmdResult {
    execute_service_operation_sync(ServiceStatus::ForceReinstallRequired, "SERVICE_REPAIR_FAILED").await
}

#[tauri::command]
pub fn take_service_repair_notice() -> bool {
    crate::core::service::take_service_repair_notice()
}

#[tauri::command]
pub fn sync_runtime_providers() {
    if matches!(*CoreManager::global().get_running_mode(), RunningMode::Service) {
        request_runtime_provider_sync(timing::RUNTIME_PROVIDER_SETTLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installer's elevation failure is recognised and given its own code.
    ///
    /// This is the failure a Linux student hits when `pkexec` has no polkit agent
    /// (or a password prompt is dismissed): the installer reports only
    /// "elevated installer failed with <status>", which used to surface as a
    /// generic install failure and send support looking for a service problem
    /// that did not exist.
    #[test]
    fn the_installers_elevation_failure_is_classified() {
        let error = anyhow::anyhow!("elevated installer failed with exit status: 1");
        let failure = elevation_failure(&error).expect("must recognise the elevation failure");
        assert_eq!(failure.code.as_deref(), Some(ELEVATION_FAILED_CODE));
        assert_eq!(failure.detail, ELEVATION_FAILED_MESSAGE);
    }

    /// It is matched anywhere in the context chain, not only at the top.
    ///
    /// `perform()` logs and re-wraps, so the installer's sentence can sit under
    /// several layers of context by the time it reaches this classifier.
    #[test]
    fn the_elevation_failure_is_found_through_context_layers() {
        let error = anyhow::anyhow!("elevated installer failed with exit status: 126")
            .context("failed to install the service")
            .context("privileged service action Install did not complete");
        assert!(
            elevation_failure(&error).is_some(),
            "the classifier must look through the context chain"
        );
    }

    /// An unrelated failure must NOT be reported as an elevation problem.
    ///
    /// The happy-path risk of a string match is a false positive that tells a
    /// student to approve a permission prompt when the real problem was something
    /// else entirely. This is the assertion that keeps the classifier honest.
    #[test]
    fn an_unrelated_failure_is_not_reported_as_elevation() {
        let error = anyhow::anyhow!("service IPC socket did not appear within the timeout");
        assert!(
            elevation_failure(&error).is_none(),
            "a non-elevation failure must fall through to the caller's own code"
        );
    }

    /// The sentence must tell the student what to do, and be honest about the
    /// thing we cannot know — whether a prompt appeared at all.
    #[test]
    fn the_elevation_message_is_actionable() {
        let lowered = ELEVATION_FAILED_MESSAGE.to_lowercase();
        assert!(
            lowered.contains("approve") || lowered.contains("permission"),
            "must say what to do, got {ELEVATION_FAILED_MESSAGE:?}"
        );
        assert!(
            lowered.contains("terminal"),
            "must offer the route that works when no prompt can appear, got {ELEVATION_FAILED_MESSAGE:?}"
        );
    }

    /// An absent privileged helper names the file, and is not called elevation.
    ///
    /// The field report this comes from: the Service install showed the OS's own
    /// "VCRUNTIME140.dll was not found" dialog, and when the runtime was installed
    /// by hand it worked — so the defect on our side is that nothing told the
    /// student, or us, that a helper file or its runtime was the problem. Reporting
    /// it as `SERVICE_INSTALL_FAILED` sends support looking for a service fault.
    #[test]
    fn a_missing_helper_is_not_reported_as_an_install_or_elevation_failure() {
        let error = anyhow::Error::new(crate::core::service::MissingServiceTool {
            tool: "clash-verge-service-install.exe".to_owned(),
            path: std::path::PathBuf::from(r"C:\Program Files\Locus\resources\clash-verge-service-install.exe"),
        })
        .context("install service failed");

        let failure = classify_privileged_failure(&error, "SERVICE_INSTALL_FAILED");
        assert_eq!(failure.code.as_deref(), Some(MISSING_HELPER_CODE));
        assert!(
            failure.detail.contains("clash-verge-service-install.exe"),
            "the sentence must name the file that is missing, got {:?}",
            failure.detail
        );
        assert!(
            !failure.detail.to_lowercase().contains("permission"),
            "this is not an elevation problem and must not ask for a prompt, got {:?}",
            failure.detail
        );
    }

    /// The typed error wins over everything below it in the classifier order.
    ///
    /// Guards the ordering in [`classify_privileged_failure`]: a helper failure
    /// wrapped in context that also mentions elevation must still be reported as
    /// the missing file, because "approve the prompt" is the wrong instruction
    /// when there is nothing to launch.
    #[test]
    fn the_missing_helper_outranks_an_elevation_sentence_in_the_same_chain() {
        let error = anyhow::Error::new(crate::core::service::MissingServiceTool {
            tool: "clash-verge-service-uninstall.exe".to_owned(),
            path: std::path::PathBuf::from("/tmp/absent"),
        })
        .context("elevated installer failed with exit status: 1");

        assert_eq!(
            classify_privileged_failure(&error, "SERVICE_INSTALL_FAILED")
                .code
                .as_deref(),
            Some(MISSING_HELPER_CODE)
        );
    }

    /// A helper that exists but could not load names the runtime, not a missing file.
    ///
    /// This is the branch the reported machine would have taken had the check been
    /// in place: the file was there, the loader could not resolve its dependencies,
    /// and the OS said so before any code of ours ran.
    #[test]
    fn a_helper_that_could_not_start_names_the_runtime_prerequisite() {
        let error = anyhow::anyhow!("failed to launch the service installer")
            .context(crate::core::service::MISSING_RUNTIME_CONTEXT);

        let failure = classify_privileged_failure(&error, "SERVICE_INSTALL_FAILED");
        assert_eq!(failure.code.as_deref(), Some(MISSING_HELPER_CODE));
        let lowered = failure.detail.to_lowercase();
        assert!(
            lowered.contains("visual c++") || lowered.contains("runtime"),
            "must name the prerequisite, got {:?}",
            failure.detail
        );
        assert!(
            lowered.contains("reinstall") || lowered.contains("visual c++"),
            "must offer a route, got {:?}",
            failure.detail
        );
    }

    /// An ordinary service failure still falls through to the caller's own code.
    ///
    /// Without this, every arm above could be widest-match and every install error
    /// would be misreported as a missing helper.
    #[test]
    fn an_ordinary_failure_keeps_the_callers_code() {
        let error =
            anyhow::anyhow!("service IPC socket did not appear within the timeout").context("install service failed");
        assert_eq!(
            classify_privileged_failure(&error, "SERVICE_INSTALL_FAILED")
                .code
                .as_deref(),
            Some("SERVICE_INSTALL_FAILED")
        );
    }
}
