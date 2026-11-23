# PROSE — every user-facing string, in one place

```
audience:    whoever edits the app's wording
status:      live
authoritative-for: the text a student sees, where it appears, and where it lives in code
verified-against: generated from src/locales/en/ and clash-verge-i18n/locales/en.yml
```

> **This file is generated. Edit the wording here, then run
> `node scripts/apply-prose.mjs`** — that writes your text back over the
> original in every locale bundle and verifies nothing was missed. Do not
> hand-edit a locale JSON; the next generation would show your change as a
> diff to itself.
>
> `node scripts/prose-manifest.mjs` regenerates this file after the code
> changes. `--check` fails if it is stale.

**321 strings.** Each row is: the key, the text, where in the app a
student meets it, and the exact `file:line` that references it.

Changing a row changes that string only. To change what a whole screen
says, edit the rows under that screen's heading.

## Activation gate

*Shown before activation, in place of the whole app (main.tsx)* — 7 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.activation.alphabet` | The code is 15 characters and never contains 0, O, 1 or I. | src/pages/activation.tsx:408 |
| `home.components.connection.activation.backupRoute` | Using the Chrysalis backup route to reach the server. | src/pages/activation.tsx:267 |
| `home.components.connection.activation.codeLabel` | Activation code | src/pages/activation.tsx:327 |
| `home.components.connection.activation.codePlaceholder` | RQ-XXXX-XXXX-XXXX-X | src/pages/activation.tsx:325 |
| `home.components.connection.activation.lede` | Your code is on the card you were sold. | src/pages/activation.tsx:306 |
| `home.components.connection.activation.submit` | Activate | src/pages/activation.tsx:399 |
| `home.components.connection.activation.title` | Enter your code | src/pages/activation.tsx:252 |

## Suspension screen

*Shown when the hub has withdrawn this device (main.tsx)* — 6 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.suspended.lapsedAction` | Contact whoever sold you the code to renew it. | src/pages/suspended.tsx:137 |
| `home.components.connection.suspended.lapsedBody` | Your code has expired, so the connection was stopped. Renewing with whoever sold you the code restores access. | src/pages/suspended.tsx:116 |
| `home.components.connection.suspended.lapsedTitle` | Your subscription has ended | src/pages/suspended.tsx:100 |
| `home.components.connection.suspended.suspendedAction` | Ask them to lift the suspension. You can connect again without re-entering your code or restarting the app. | src/pages/suspended.tsx:138 |
| `home.components.connection.suspended.suspendedBody` | An operator has suspended this code, so the connection was stopped. Your code is still saved on this computer. | src/pages/suspended.tsx:117 |
| `home.components.connection.suspended.suspendedTitle` | This device has been signed out | src/pages/suspended.tsx:101 |

## Connection tab

*The main screen — status, Connect, traffic, tier (pages/connection.tsx)* — 29 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.activeConnections` | Active connections | src/pages/connection.tsx:549 |
| `home.components.connection.blockerServiceInstallRequired` | Chrysalis cannot turn the VPN on yet: its helper service is not installed, and the permission prompt to install it was dismissed, denied, or never appeared. Quit and reopen Chrysalis, then approve the prompt when it appears. If no prompt appears at all, reinstall Chrysalis, then approve the prompt. | src/pages/connect-notice.ts:149 |
| `home.components.connection.blockerServiceOperationInFlight` | Chrysalis is setting up its helper service. This takes a moment — wait for the prompt or for this message to clear, then try again. | src/pages/connect-notice.ts:151 |
| `home.components.connection.blockerServiceUnavailable` | Chrysalis cannot turn the VPN on yet: its helper service is installed but not working ({{reason}}). Quit and reopen Chrysalis to repair it. If that does not work, reinstall Chrysalis. | src/pages/connect-notice.ts:150 |
| `home.components.connection.cancel` | Stop | src/pages/connection.tsx:149 |
| `home.components.connection.checking` | Checking… | src/pages/connection.tsx:147 |
| `home.components.connection.connect` | Connect | src/pages/connection.tsx:154 |
| `home.components.connection.connected` | Connected | src/pages/connection.tsx:174 |
| `home.components.connection.connecting` | Connecting… | src/pages/connection.tsx:172 |
| `home.components.connection.disconnect` | Disconnect | src/pages/connection.tsx:153 |
| `home.components.connection.disconnecting` | Disconnecting… | src/pages/connection.tsx:151 |
| `home.components.connection.freeData` | Free data | src/components/connection/usage-bar.tsx:49 |
| `home.components.connection.freeDataNearlyOut` | {{used}} of {{total}} used — nearly out of free data | src/components/connection/usage-bar.tsx:87 |
| `home.components.connection.freeDataThrottled` | {{used}} of {{total}} used — free data is spent for this month | src/components/connection/usage-bar.tsx:88 |
| `home.components.connection.freeDataThrottledUpgrade` | Full is faster and unmetered. Ask the person who sold you your code about upgrading. | src/components/connection/usage-bar.tsx:110 |
| `home.components.connection.freeDataUsed` | {{used}} of {{total}} used | src/components/connection/usage-bar.tsx:86 |
| `home.components.connection.metricPending` | Not reported yet | src/pages/connection.tsx:726 |
| `home.components.connection.metricsWhenConnected` | Traffic figures appear once the VPN is on. | src/pages/connection.tsx:485 |
| `home.components.connection.needsActivation` | This device needs an activation code before it can connect. | src/pages/connection.tsx:340 |
| `home.components.connection.notConnected` | Not connected | src/pages/connection.tsx:175 |
| `home.components.connection.requiresSetup` | Chrysalis cannot turn the VPN on yet: it needs permission, and its helper service is not installed. Chrysalis will ask for that permission when it starts — approve the prompt, then try again. | src/pages/connection.tsx:431 |
| `home.components.connection.sessionDownload` | Downloaded this session | src/components/connection/traffic-summary.tsx:82<br>src/pages/connection.tsx:535 |
| `home.components.connection.sessionUpload` | Uploaded this session | src/components/connection/traffic-summary.tsx:75<br>src/pages/connection.tsx:528 |
| `home.components.connection.subscriptionExpiresIn` | Your Chrysalis subscription expires in {{count}} days. Contact the person who sold you the code to renew it. | src/pages/connection.tsx:404 |
| `home.components.connection.subscriptionExpiresToday` | Your Chrysalis subscription expires today. Contact the person who sold you the code to renew it. | src/pages/connection.tsx:403 |
| `home.components.connection.subscriptionLapsed` | Your Chrysalis subscription has expired, so the VPN will not connect. Contact the person who sold you the code to renew it. | src/pages/connection.tsx:389 |
| `home.components.connection.supportEmail` | maillcontainer@proton.me | src/components/shared/support-block.tsx:100 |
| `home.components.connection.takingLonger` | Still connecting. This can take a few seconds on a school network — you can stop and try again. | src/pages/connection.tsx:286 |
| `home.components.connection.toggleHint` | Press Enter to turn the VPN on or off | src/pages/connection.tsx:266 |

## Support block

*The email-us panel, on activation, Connection and Account* — 7 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.supportBlock.deviceIdHint` | Include the Device ID from your Account page — it is the one thing that tells us which install you are using. | src/components/shared/support-block.tsx:119 |
| `home.components.connection.supportBlock.emailLead` | If they cannot, or you bought your code yourself, email | src/components/shared/support-block.tsx:98 |
| `home.components.connection.supportBlock.emailNote` | and describe the problem: what you were doing, the exact message the app showed, and the Device ID from your Account page. Short is fine — the message is the part that matters. | src/components/shared/support-block.tsx:111 |
| `home.components.connection.supportBlock.middleman` | Start with the person who sold you your code: they know your account and can usually fix it on the spot. | src/components/shared/support-block.tsx:90 |
| `home.components.connection.supportBlock.replyPromise` | Every email is answered within 12 hours, not counting nighttime. | src/components/shared/support-block.tsx:136 |
| `home.components.connection.supportBlock.title` | Any problem at all — email us. | src/components/shared/support-block.tsx:82 |
| `home.components.connection.supportBlock.why` | Chrysalis is run by one person, so an email that names the error is something that can actually be fixed. | src/components/shared/support-block.tsx:143 |

## Sign-out card

*Bottom of Account — sign out, its dialog, its outcomes* — 16 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.signOut.action` | Sign out on this device | src/components/shared/sign-out-card.tsx:218 |
| `home.components.connection.signOut.body` | Signing out makes this computer forget your access key. Your subscription is not cancelled, and the time you have paid for keeps running. | src/components/shared/sign-out-card.tsx:187 |
| `home.components.connection.signOut.confirmBody` | Chrysalis will forget your access key on this computer. This computer will stop working with Chrysalis until you enter the key from your card again. | src/components/shared/sign-out-card.tsx:253 |
| `home.components.connection.signOut.confirmKeeps` | Your subscription is not cancelled, and it will not be shortened. | src/components/shared/sign-out-card.tsx:256 |
| `home.components.connection.signOut.confirmMoving` | If you are moving to a new computer, this is what you want. | src/components/shared/sign-out-card.tsx:263 |
| `home.components.connection.signOut.confirmTitle` | Sign out on this device? | src/components/shared/sign-out-card.tsx:249 |
| `home.components.connection.signOut.countingDown` | Signing out in {{count}}… | src/components/shared/sign-out-card.tsx:284 |
| `home.components.connection.signOut.deviceIdCopied` | Device ID copied | src/pages/account.tsx:336 |
| `home.components.connection.signOut.deviceIdCopy` | Copy | src/pages/account.tsx:350 |
| `home.components.connection.signOut.deviceIdCopyFailed` | Could not copy automatically — select the Device ID and copy it by hand. | src/pages/account.tsx:345 |
| `home.components.connection.signOut.signedOut` | Signed out. This computer no longer holds your key. | src/components/shared/sign-out-card.tsx:162 |
| `home.components.connection.signOut.signedOutFailed` | Chrysalis could not sign out. Nothing has changed on this device. Try again, and email us if it keeps happening. | src/components/shared/sign-out-card.tsx:120 |
| `home.components.connection.signOut.signedOutOffline` | Signed out on this computer. We could not reach Chrysalis to record it — your key is still yours, and entering it on your next device will work. | src/components/shared/sign-out-card.tsx:164 |
| `home.components.connection.signOut.signedOutReleased` | Signed out. Your key has been released here and is ready for your next device. | src/components/shared/sign-out-card.tsx:160 |
| `home.components.connection.signOut.title` | Signed in on this device | src/components/shared/sign-out-card.tsx:175 |
| `home.components.connection.signOut.undo` | Cancel | src/components/shared/sign-out-card.tsx:271 |

## Account tab

*Subscription, device, preferences, updates (pages/account.tsx)* — 38 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.connection.account.activateForExpiry` | Activate this device to see your subscription. | src/pages/account.tsx:206 |
| `home.components.connection.account.appVersion` | App version | src/pages/account.tsx:358 |
| `home.components.connection.account.autoUpdate` | Check for updates automatically | src/pages/account.tsx:459 |
| `home.components.connection.account.checkingStatus` | Checking… | src/pages/account.tsx:282 |
| `home.components.connection.account.checkStatus` | Check status now | src/pages/account.tsx:283 |
| `home.components.connection.account.confirmedDaysAgo` | {{count}} d ago | src/pages/account.tsx:117 |
| `home.components.connection.account.confirmedHoursAgo` | {{count}} h ago | src/pages/account.tsx:114 |
| `home.components.connection.account.confirmedJustNow` | just now | src/pages/account.tsx:106 |
| `home.components.connection.account.confirmedMinutesAgo` | {{count}} min ago | src/pages/account.tsx:109 |
| `home.components.connection.account.daysLeft` | {{count}} days | src/pages/account.tsx:155 |
| `home.components.connection.account.deviceId` | Device ID | src/pages/account.tsx:308 |
| `home.components.connection.account.deviceIdHint` | Quote the device ID if you need support — it identifies this install without revealing anything about you. | src/pages/account.tsx:366 |
| `home.components.connection.account.expired` | Expired | src/pages/account.tsx:168 |
| `home.components.connection.account.expires` | Expires | src/pages/account.tsx:149 |
| `home.components.connection.account.expiresSoon` | Your subscription expires soon. Contact the person who sold you the code to renew. | src/pages/account.tsx:220 |
| `home.components.connection.account.hubBehind` | Your Chrysalis server is running an older version of its account checks. If someone suspends your access, this device will not find out until the server is updated. Quote this number to them: {{hub}}. | src/pages/account.tsx:387 |
| `home.components.connection.account.language` | Language | src/pages/account.tsx:432 |
| `home.components.connection.account.lastChecked` | Last confirmed {{time}} | src/pages/account.tsx:287 |
| `home.components.connection.account.noExpiryYet` | Your subscription end date is not available yet. It will appear after this device next contacts Chrysalis. | src/pages/account.tsx:205 |
| `home.components.connection.account.notActivated` | This device is not activated. Enter the code from your card to connect. | src/pages/account.tsx:664 |
| `home.components.connection.account.notActivatedToCheck` | Activate a code first — there is nothing to confirm yet. | src/pages/account.tsx:272 |
| `home.components.connection.account.off` | Off | src/pages/account.tsx:472 |
| `home.components.connection.account.on` | On | src/pages/account.tsx:471 |
| `home.components.connection.account.oneDayLeft` | 1 day | src/pages/account.tsx:154 |
| `home.components.connection.account.plan` | Plan | src/pages/account.tsx:132 |
| `home.components.connection.account.platform` | Platform | src/pages/account.tsx:354 |
| `home.components.connection.account.preferences` | Preferences | src/pages/account.tsx:426 |
| `home.components.connection.account.refresh` | Refresh | src/pages/account.tsx:635 |
| `home.components.connection.account.refreshBusy` | A status read was already running | src/pages/account.tsx:626 |
| `home.components.connection.account.refreshed` | Up to date — status re-read | src/pages/account.tsx:624 |
| `home.components.connection.account.refreshFailed` | Could not re-read the status | src/pages/account.tsx:627 |
| `home.components.connection.account.storedCodeDamaged` | The sign-in saved on this computer is damaged and could not be used, so Chrysalis is asking you to enter your code again. Your plan and its remaining time are not affected. | src/pages/account.tsx:663 |
| `home.components.connection.account.subscription` | Subscription | src/pages/account.tsx:128 |
| `home.components.connection.account.thisDevice` | This device | src/pages/account.tsx:305 |
| `home.components.connection.account.today` | Today | src/pages/account.tsx:152 |
| `home.components.connection.account.updateBlocked` | Chrysalis cannot update itself right now: {{reason}} | src/pages/account.tsx:539 |
| `home.components.connection.account.updateCheckOff` | Automatic update checking is off, so Chrysalis is not being told about new versions. Turn it on above to receive fixes. | src/pages/account.tsx:504 |
| `home.components.connection.account.updateCheckOffAction` | Turn on automatic updates | src/pages/account.tsx:521 |

## Traffic graph and figures

*The sidebar graph and the Connection/Account traffic cards* — 11 strings.

| Key | Text | Code |
|---|---|---|
| `home.components.traffic.chartStyles.linear` | Linear | src/components/connection/enhanced-canvas-traffic-graph.tsx:1159 |
| `home.components.traffic.chartStyles.smooth` | Smooth | src/components/connection/enhanced-canvas-traffic-graph.tsx:1158 |
| `home.components.traffic.diagnostics` | Points: {{points}} \| Compressed: {{compressed}} | src/components/connection/enhanced-canvas-traffic-graph.tsx:1189 |
| `home.components.traffic.idle` | Idle | src/pages/connection.tsx:637 |
| `home.components.traffic.legends.download` | Download | src/components/connection/enhanced-canvas-traffic-graph.tsx:1142 |
| `home.components.traffic.legends.upload` | Upload | src/components/connection/enhanced-canvas-traffic-graph.tsx:1132 |
| `home.components.traffic.metrics.downloadSpeed` | Download Speed | src/components/layout/layout-traffic.tsx:108<br>src/pages/connection.tsx:492 |
| `home.components.traffic.metrics.memoryUsage` | VPN memory | src/components/layout/layout-traffic.tsx:126<br>src/pages/connection.tsx:542 |
| `home.components.traffic.metrics.uploadSpeed` | Upload Speed | src/components/layout/layout-traffic.tsx:91<br>src/pages/connection.tsx:497 |
| `home.components.traffic.patterns.minutes` | {{time}} Minutes | src/components/connection/enhanced-canvas-traffic-graph.tsx:1036 |
| `home.components.traffic.unknownTime` | Unknown time | src/components/connection/enhanced-canvas-traffic-graph.tsx:338 |

## Sidebar and navigation

*Tabs, the subscription chip, the window menu* — 8 strings.

| Key | Text | Code |
|---|---|---|
| `layout.components.navigation.menu.collapseNavBar` | Collapse navigation bar | src/components/layout/layout-sidebar.tsx:215 |
| `layout.components.navigation.menu.expandNavBar` | Expand navigation bar | src/components/layout/layout-sidebar.tsx:214 |
| `layout.components.navigation.menu.lock` | Lock menu order | src/components/layout/layout-sidebar.tsx:222 |
| `layout.components.navigation.menu.reorderMode` | Menu reorder mode | src/components/layout/layout-sidebar.tsx:181 |
| `layout.components.navigation.menu.restoreDefaultOrder` | Restore default order | src/components/layout/layout-sidebar.tsx:230 |
| `layout.components.navigation.menu.unlock` | Unlock menu order | src/components/layout/layout-sidebar.tsx:223 |
| `layout.components.navigation.tabs.home` | Home | src/pages/_navigation-meta.ts:18 |
| `layout.components.navigation.tabs.settings` | Settings | src/pages/_navigation-meta.ts:22 |

## Dialogs

*Service migration, system-proxy permission, subscription, update prompt* — 35 strings.

| Key | Text | Code |
|---|---|---|
| `layout.components.serviceMigration.checkingMessage` | Checking the system service status and current running mode. Please wait… | src/components/layout/service-migration-dialog.tsx:156 |
| `layout.components.serviceMigration.errors.actionFailed` | The service operation failed | src/components/layout/service-migration-dialog.tsx:90 |
| `layout.components.serviceMigration.errors.restartFailed` | Failed to restart the core | src/components/layout/service-migration-dialog.tsx:116 |
| `layout.components.serviceMigration.errors.stateRefreshFailed` | Failed to refresh the service state | src/components/layout/service-migration-dialog.tsx:101 |
| `layout.components.serviceMigration.locationRefusedMessage` | The service cannot start the VPN because its installation path or directory permissions failed a security check. Reinstall the service to restore its files and permissions, then try again. | src/components/layout/sysproxy-privilege-dialog.tsx:36 |
| `layout.components.serviceMigration.message` | The Chrysalis service version does not match the version required by this app. Choose Reinstall Service to uninstall the existing service and install the required version. | src/components/layout/service-migration-dialog.tsx:158 |
| `layout.components.serviceMigration.reinstall` | Reinstall service | src/components/layout/service-migration-dialog.tsx:147<br>src/components/layout/sysproxy-privilege-dialog.tsx:143 |
| `layout.components.serviceMigration.repair` | Repair service | src/components/layout/service-migration-dialog.tsx:146<br>src/components/layout/sysproxy-privilege-dialog.tsx:42 |
| `layout.components.serviceMigration.success` | The Chrysalis service was updated successfully | src/components/layout/service-migration-dialog.tsx:133<br>src/components/layout/sysproxy-privilege-dialog.tsx:119 |
| `layout.components.serviceMigration.title` | Service update required | src/components/layout/service-migration-dialog.tsx:141 |
| `layout.components.serviceMigration.unavailableMessage` | The Chrysalis service is required and is not available. Install or repair it, then try again. Chrysalis cannot turn the VPN on without it. | src/components/layout/service-migration-dialog.tsx:159 |
| `layout.components.subscription.daysLeft` | Expires in {{count}} days | src/components/layout/layout-subscription.tsx:96 |
| `layout.components.subscription.expired` | Subscription expired | src/components/layout/layout-subscription.tsx:74 |
| `layout.components.subscription.expiredHint` | Your Chrysalis subscription has ended. Contact the person who sold you the code to renew it, then activate again. | src/components/layout/layout-subscription.tsx:75 |
| `layout.components.subscription.expiresToday` | Expires today | src/components/layout/layout-subscription.tsx:93 |
| `layout.components.subscription.hint` | When your Chrysalis subscription ends. | src/components/layout/layout-subscription.tsx:111 |
| `layout.components.subscription.oneDayLeft` | Expires in 1 day | src/components/layout/layout-subscription.tsx:95 |
| `layout.components.subscription.unconfirmed` | Subscription status unavailable | src/components/layout/layout-subscription.tsx:84 |
| `layout.components.subscription.unconfirmedHint` | Chrysalis has not confirmed your subscription with the server yet. Connect once on a working network and this will update. | src/components/layout/layout-subscription.tsx:85 |
| `layout.components.sysproxyPrivilege.alternative` | If you would rather not install the service, turning system proxy off in Chrysalis also stops this prompt. | src/components/layout/sysproxy-privilege-dialog.tsx:162 |
| `layout.components.sysproxyPrivilege.applying` | Applying your system proxy change… | src/components/layout/sysproxy-privilege-dialog.tsx:75 |
| `layout.components.sysproxyPrivilege.installing` | Installing the system service. This may ask for permission. | src/components/layout/sysproxy-privilege-dialog.tsx:73 |
| `layout.components.sysproxyPrivilege.later` | Later | src/components/layout/sysproxy-privilege-dialog.tsx:148 |
| `layout.components.sysproxyPrivilege.message` | Chrysalis was not allowed to change the system proxy. Installing the Chrysalis service lets it make proxy changes with your permission, and this prompt will not come back. | src/components/layout/sysproxy-privilege-dialog.tsx:33 |
| `layout.components.sysproxyPrivilege.restarting` | Service installed. Restarting the core through it… | src/components/layout/sysproxy-privilege-dialog.tsx:74 |
| `layout.components.sysproxyPrivilege.title` | Could not set the system proxy | src/components/layout/sysproxy-privilege-dialog.tsx:40 |
| `layout.components.sysproxyPrivilege.tunMessage` | TUN mode needs the Chrysalis service to take over network traffic. Once it is installed, TUN mode will be turned on for you. | src/components/layout/sysproxy-privilege-dialog.tsx:34 |
| `layout.components.sysproxyPrivilege.tunTitle` | TUN mode needs the system service | src/components/layout/sysproxy-privilege-dialog.tsx:41 |
| `shared.update.body` | Chrysalis {{version}} is ready to install. The update takes a moment and your connection will drop while it installs. | src/components/layout/update-prompt.tsx:214 |
| `shared.update.connectionNotice` | You can keep using Chrysalis until you install. | src/components/layout/update-prompt.tsx:251 |
| `shared.update.downloading` | Downloading… | src/components/layout/update-prompt.tsx:225 |
| `shared.update.downloadingPercent` | Downloading… {{percent}}% | src/components/layout/update-prompt.tsx:226 |
| `shared.update.installNow` | Install now | src/components/layout/update-prompt.tsx:199 |
| `shared.update.later` | Later | src/components/layout/update-prompt.tsx:200 |
| `shared.update.restartNotice` | Chrysalis may close and reopen by itself to finish. Do not force-quit it. | src/components/layout/update-prompt.tsx:235 |

## Toasts and notifications

*Transient messages: errors, successes, warnings. The `settings.*` keys here are emitted by the profile engine through `pages/_layout/utils/notification-handlers.ts`, not by a Settings tab that no longer exists.* — 61 strings.

| Key | Text | Code |
|---|---|---|
| `layout.feedback.errors.quitCoreStopFailed` | The core could not be stopped safely, so quitting was cancelled | src/pages/_layout/utils/notification-handlers.ts:82 |
| `layout.feedback.errors.restartCoreStopFailed` | The core could not be stopped safely, so the restart was cancelled | src/pages/_layout/utils/notification-handlers.ts:77 |
| `settings.feedback.errors.clash.configUpdateFailed` | Failed to update core configuration | src/services/notice-service.ts:62 |
| `settings.feedback.errors.clash.modeUpdateFailed` | Failed to change proxy mode | src/services/notice-service.ts:63 |
| `settings.feedback.errors.clash.restartFailed` | Failed to restart the core | src/services/notice-service.ts:65 |
| `settings.feedback.errors.clash.startFailed` | Failed to start the core | src/services/notice-service.ts:66 |
| `settings.feedback.errors.clash.stopFailed` | Failed to stop the core | src/services/notice-service.ts:67 |
| `settings.feedback.errors.clashService.installFailed` | Failed to install the service | src/services/notice-service.ts:77 |
| `settings.feedback.errors.clashService.reinstallFailed` | Failed to reinstall the service | src/services/notice-service.ts:79 |
| `settings.feedback.errors.clashService.repairFailed` | Failed to repair the service | src/services/notice-service.ts:80 |
| `settings.feedback.errors.clashService.uninstallFailed` | Failed to uninstall the service | src/services/notice-service.ts:82 |
| `settings.feedback.errors.sysproxy.coreNotReady` | The system proxy could not be enabled because the VPN is not running. Turn the VPN on and try again. | src/services/notice-service.ts:83 |
| `settings.feedback.errors.sysproxy.directFallback` | Failed to set the system proxy; traffic has been switched back to a direct connection. | src/services/notice-service.ts:84 |
| `settings.feedback.errors.sysproxy.guardStopped` | The system proxy guard stopped after failing several times running. The system proxy is no longer being kept in place. | src/services/notice-service.ts:85 |
| `settings.feedback.errors.sysproxy.privilegeRequired` | Cannot set the system proxy: Chrysalis needs permission for that change. Install the Chrysalis service and try again — this prompt will not come back. | src/services/notice-service.ts:87 |
| `settings.feedback.errors.sysproxy.systemCallFailed` | The system proxy change did not go through. This is usually temporary, so try again. | src/services/notice-service.ts:89 |
| `settings.feedback.notifications.clash.changeFailed` | Failed to change core | src/pages/_layout/utils/notification-handlers.ts:143<br>src/services/notice-service.ts:64 |
| `settings.feedback.notifications.clash.changeSuccess` | Core changed successfully | src/pages/_layout/utils/notification-handlers.ts:138 |
| `settings.feedback.notifications.clash.restartSuccess` | Clash Core Restarted | src/hooks/use-service-installer.ts:34 |
| `settings.feedback.notifications.clashService.installSuccess` | Service Installed Successfully | src/hooks/use-service-installer.ts:28 |
| `settings.feedback.notifications.clashService.uninstallSuccess` | Service Uninstalled Successfully | src/hooks/use-service-uninstaller.ts:25 |
| `settings.feedback.notifications.updater.withClashProxyFailed` | Update failed even with Clash proxy | src/pages/_layout/utils/notification-handlers.ts:92 |
| `settings.feedback.notifications.updater.withClashProxySuccess` | Update with Clash proxy successfully | src/pages/_layout/utils/notification-handlers.ts:87 |
| `settings.modals.clashPort.messages.automaticFallback` | Port {{oldPort}} was in use, so this session runs on {{newPort}}. Your setting is unchanged and {{oldPort}} is retried on the next start. | src/pages/_layout/utils/notification-handlers.ts:148 |
| `settings.modals.clashPort.messages.automaticFallbackFailed` | Automatic mixed proxy port fallback failed: {{error}} | src/pages/_layout/utils/notification-handlers.ts:155 |
| `settings.modals.clashPort.messages.portTooHigh` | The port must be 65535 or lower | src/hooks/use-clash.ts:56 |
| `settings.modals.clashPort.messages.portTooLow` | The port must be 1000 or higher | src/hooks/use-clash.ts:51 |
| `settings.modals.dns.protection.autoDisabled` | DNS Overwrite was disabled to preserve the custom DNS settings in the current subscription. | src/pages/_layout/utils/notification-handlers.ts:44 |
| `settings.sections.proxyControl.actions.installService` | Install Service | src/components/layout/service-migration-dialog.tsx:144<br>src/components/layout/sysproxy-privilege-dialog.tsx:145 |
| `settings.sections.proxyControl.actions.switchToServiceMode` | Switch to service mode | src/components/layout/sysproxy-privilege-dialog.tsx:146 |
| `settings.sections.proxyControl.messages.installedCheckProxy` | The service is installed and the core is running through it. Check the system proxy switch and make your change again if it did not take. | src/components/layout/sysproxy-privilege-dialog.tsx:121 |
| `settings.sections.proxyControl.messages.installedCoreNotOnService` | The service is installed, but the core is not running through it. Check the core and try again. | src/components/layout/sysproxy-privilege-dialog.tsx:127 |
| `settings.sections.proxyControl.messages.installedProxyRestored` | The service is installed, the core is running through it, and your system proxy change has been applied. | src/components/layout/sysproxy-privilege-dialog.tsx:122 |
| `settings.sections.system.fields.autoLaunch` | Auto Launch | src/pages/account.tsx:556 |
| `settings.sections.system.notifications.tunMode.autoDisabled` | TUN Mode automatically disabled due to service unavailable | src/pages/_layout/utils/notification-handlers.ts:69 |
| `settings.sections.system.notifications.tunMode.autoDisableFailed` | Failed to disable TUN Mode automatically | src/pages/_layout/utils/notification-handlers.ts:73 |
| `settings.statuses.clash.restarting` | Restarting Core... | src/hooks/use-service-installer.ts:33 |
| `settings.statuses.clashService.installing` | Installing Service... | src/hooks/use-service-installer.ts:27 |
| `settings.statuses.clashService.uninstalling` | Uninstalling Service... | src/hooks/use-service-uninstaller.ts:21 |
| `shared.feedback.notices.prefixedRaw` | {{prefix}} {{message}} | src/components/layout/notice-manager.tsx:136<br>src/services/notice-service.ts:353 |
| `shared.feedback.notices.raw` | {{message}} | src/components/layout/notice-manager.tsx:137<br>src/services/notice-service.ts:255 |
| `shared.feedback.notifications.common.copySuccess` | Copy Success | src/components/layout/notice-manager.tsx:177 |
| `shared.feedback.notifications.importSubscriptionSuccess` | Import subscription successful | src/pages/_layout/utils/notification-handlers.ts:19 |
| `shared.feedback.notifications.updateAvailable` | Update Available | src/components/layout/update-prompt.tsx:198 |
| `shared.feedback.validation.config.bootFailed` | Boot subscription configuration validation failed. Started with the default configuration; please check the subscription configuration file. | src/pages/_layout/utils/notification-handlers.ts:98 |
| `shared.feedback.validation.config.coreChangeFailed` | Configuration validation failed when switching the kernel. Started with the default configuration; please check the subscription configuration file. | src/pages/_layout/utils/notification-handlers.ts:101 |
| `shared.feedback.validation.config.failed` | Subscription configuration validation failed. Please check the subscription configuration file; modifications have been rolled back. | src/pages/_layout/utils/notification-handlers.ts:105 |
| `shared.feedback.validation.config.processTerminated` | The validation process was terminated. Please check whether security software is preventing the core from running, or whether your computer's CPU and runtime environment support the core executable. | src/pages/_layout/utils/notification-handlers.ts:107 |
| `shared.feedback.validation.merge.generalError` | Merge file error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:135 |
| `shared.feedback.validation.merge.keyError` | Merge file key error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:133 |
| `shared.feedback.validation.merge.mappingError` | Merge file mapping error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:131 |
| `shared.feedback.validation.merge.syntaxError` | Merge file syntax error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:129 |
| `shared.feedback.validation.script.fileError` | Script file error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:111 |
| `shared.feedback.validation.script.fileNotFound` | File missing, changes reverted | src/pages/_layout/utils/notification-handlers.ts:117 |
| `shared.feedback.validation.script.missingMain` | Script error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:115 |
| `shared.feedback.validation.script.syntaxError` | Script syntax error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:113 |
| `shared.feedback.validation.yaml.generalError` | YAML error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:127 |
| `shared.feedback.validation.yaml.keyError` | YAML key error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:125 |
| `shared.feedback.validation.yaml.mappingError` | YAML mapping error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:123 |
| `shared.feedback.validation.yaml.readError` | YAML read error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:121 |
| `shared.feedback.validation.yaml.syntaxError` | YAML syntax error, changes reverted | src/pages/_layout/utils/notification-handlers.ts:119 |

## Error screen

*What a crash renders instead of the page* — 10 strings.

| Key | Text | Code |
|---|---|---|
| `shared.feedback.errors.componentStack` | Component stack | src/components/shared/traffic-error-boundary.tsx:248 |
| `shared.feedback.errors.details` | Error details | src/components/shared/traffic-error-boundary.tsx:229 |
| `shared.feedback.errors.label` | Error | src/components/shared/traffic-error-boundary.tsx:177 |
| `shared.feedback.errors.operationFailed` | The operation failed | src/services/notice-service.ts:325 |
| `shared.feedback.errors.stack` | Error stack | src/components/base/base-error-boundary.tsx:16 |
| `shared.feedback.errors.trafficStats` | Traffic Statistics Error | src/components/shared/traffic-error-boundary.tsx:164 |
| `shared.feedback.errors.trafficStatsDescription` | The traffic statistics component encountered an error and has been disabled to prevent crashes. | src/components/shared/traffic-error-boundary.tsx:172 |
| `shared.feedback.errors.trafficUnavailable` | Traffic data unavailable | src/components/shared/traffic-error-boundary.tsx:293 |
| `shared.feedback.errors.unexpected` | Something went wrong | src/components/base/base-error-boundary.tsx:12 |
| `shared.feedback.errors.unknown` | Unknown error | src/components/shared/traffic-error-boundary.tsx:180 |

## Native strings

*Tray menu, OS notifications, macOS launch guard — Rust + YAML* — 68 strings.

| Key | Text | Code |
|---|---|---|
| `backend:notifications.dashboardToggled.title` | Dashboard | client/src-tauri/src/utils/notification.rs:39 |
| `backend:notifications.dashboardToggled.body` | Dashboard visibility has been updated. | client/src-tauri/src/utils/notification.rs:40 |
| `backend:notifications.clashModeChanged.title` | Mode Switch | client/src-tauri/src/utils/notification.rs:44 |
| `backend:notifications.clashModeChanged.body` | Switched to {mode}. | client/src-tauri/src/utils/notification.rs:45 |
| `backend:notifications.systemProxyToggled.title` | System Proxy | client/src-tauri/src/utils/notification.rs:51 |
| `backend:notifications.systemProxyToggled.on` | System proxy has been enabled. | client/src-tauri/src/utils/notification.rs:53 |
| `backend:notifications.systemProxyToggled.off` | System proxy has been disabled. | client/src-tauri/src/utils/notification.rs:55 |
| `backend:notifications.systemProxyFailed.title` | System Proxy | client/src-tauri/src/utils/notification.rs:62 |
| `backend:notifications.systemProxyFailed.body` | The system proxy could not be set. Open Clash Verge to see why. | client/src-tauri/src/utils/notification.rs:63 |
| `backend:notifications.systemProxyGuardStopped.title` | System Proxy Guard | client/src-tauri/src/utils/notification.rs:67 |
| `backend:notifications.systemProxyGuardStopped.body` | The guard stopped after failing several times running. Open Clash Verge to see why. | client/src-tauri/src/utils/notification.rs:68 |
| `backend:notifications.tunModeToggled.title` | TUN Mode | client/src-tauri/src/utils/notification.rs:72 |
| `backend:notifications.tunModeToggled.on` | TUN mode has been enabled. | client/src-tauri/src/utils/notification.rs:74 |
| `backend:notifications.tunModeToggled.off` | TUN mode has been disabled. | client/src-tauri/src/utils/notification.rs:76 |
| `backend:notifications.lightweightModeEntered.title` | Lightweight Mode | client/src-tauri/src/utils/notification.rs:82 |
| `backend:notifications.lightweightModeEntered.body` | Entered lightweight mode. | client/src-tauri/src/utils/notification.rs:83 |
| `backend:notifications.profilesReactivated.title` | Profiles | client/src-tauri/src/utils/notification.rs:87 |
| `backend:notifications.profilesReactivated.body` | Profile Reactivated. | client/src-tauri/src/utils/notification.rs:88 |
| `backend:notifications.appQuit.title` | About to Exit | client/src-tauri/src/utils/notification.rs:92 |
| `backend:notifications.appQuit.body` | Clash Verge is about to exit. | client/src-tauri/src/utils/notification.rs:93 |
| `backend:service.adminInstallPrompt` | Installing the Clash Verge service requires administrator privileges. | client/src-tauri/src/core/service.rs:773 |
| `backend:service.adminUninstallPrompt` | Uninstalling the Clash Verge service requires administrator privileges. | client/src-tauri/src/core/service.rs:712 |
| `backend:launchGuard.currentExeError` | Unable to determine the application location: {error} | client/src-tauri/src/utils/macos_launch_guard.rs:174 |
| `backend:launchGuard.homeDirError` | Unable to determine the current user's home directory. The application will exit: {error} | client/src-tauri/src/utils/macos_launch_guard.rs:182 |
| `backend:launchGuard.translocated` | macOS is running this application from a temporary App Translocation path. In Finder, move it manually to /Applications or ~/Applications, then reopen it. | client/src-tauri/src/utils/macos_launch_guard.rs:191 |
| `backend:launchGuard.notInBundle` | The executable is not inside an application bundle. | client/src-tauri/src/utils/macos_launch_guard.rs:197 |
| `backend:launchGuard.canonicalizeBundleError` | Unable to resolve the application bundle path: {error} | client/src-tauri/src/utils/macos_launch_guard.rs:200 |
| `backend:launchGuard.bundleNameError` | Unable to determine the application bundle name. The application will exit. | client/src-tauri/src/utils/macos_launch_guard.rs:220 |
| `backend:launchGuard.replacePrompt` | An application already exists at {destination}. Replace it and relaunch from Applications? | client/src-tauri/src/utils/macos_launch_guard.rs:227 |
| `backend:launchGuard.movePrompt` | To start the background service safely, the application must be moved to {destination}. Move and relaunch it now? | client/src-tauri/src/utils/macos_launch_guard.rs:229 |
| `backend:launchGuard.createApplicationsDirError` | Unable to create the Applications directory: {error} | client/src-tauri/src/utils/macos_launch_guard.rs:236 |
| `backend:launchGuard.moveFailed` | Failed to move the application. Move it manually in Finder and try again. | client/src-tauri/src/utils/macos_launch_guard.rs:250 |
| `backend:launchGuard.replaceFailed` | Unable to replace the existing application safely: {error} | client/src-tauri/src/utils/macos_launch_guard.rs:257 |
| `backend:launchGuard.relaunchFailed` | The application was copied, but it could not be relaunched automatically. Open it manually from Applications. | client/src-tauri/src/utils/macos_launch_guard.rs:275 |
| `backend:launchGuard.cancel` | Cancel | client/src-tauri/src/utils/macos_launch_guard.rs:336 |
| `backend:launchGuard.move` | Move | client/src-tauri/src/utils/macos_launch_guard.rs:337 |
| `backend:launchGuard.ok` | OK | client/src-tauri/src/utils/macos_launch_guard.rs:355 |
| `backend:tray.dashboard` | Dashboard | client/src-tauri/src/core/tray/menu_def.rs:28 |
| `backend:tray.ruleMode` | Rule Mode | client/src-tauri/src/core/tray/menu_def.rs:29 |
| `backend:tray.globalMode` | Global Mode | client/src-tauri/src/core/tray/menu_def.rs:30 |
| `backend:tray.directMode` | Direct Mode | client/src-tauri/src/core/tray/menu_def.rs:31 |
| `backend:tray.outboundModes` | Outbound Modes | client/src-tauri/src/core/tray/menu_def.rs:32 |
| `backend:tray.rule` | Rule | client/src-tauri/src/core/tray/mod.rs:738 |
| `backend:tray.direct` | Direct | client/src-tauri/src/core/tray/mod.rs:737 |
| `backend:tray.global` | Global | client/src-tauri/src/core/tray/mod.rs:736 |
| `backend:tray.profiles` | Profiles | client/src-tauri/src/core/tray/menu_def.rs:33 |
| `backend:tray.proxies` | Proxies | client/src-tauri/src/core/tray/menu_def.rs:34 |
| `backend:tray.systemProxy` | System Proxy | client/src-tauri/src/core/tray/menu_def.rs:35 |
| `backend:tray.tunMode` | TUN Mode | client/src-tauri/src/core/tray/menu_def.rs:36 |
| `backend:tray.connect` | Connect | client/src-tauri/src/core/tray/menu_def.rs:37 |
| `backend:tray.disconnect` | Disconnect | client/src-tauri/src/core/tray/menu_def.rs:38 |
| `backend:tray.closeAllConnections` | Close All Connections | client/src-tauri/src/core/tray/menu_def.rs:39 |
| `backend:tray.lightweightMode` | Lightweight Mode | client/src-tauri/src/core/tray/menu_def.rs:40 |
| `backend:tray.copyEnv` | Copy Environment Variables | client/src-tauri/src/core/tray/menu_def.rs:41 |
| `backend:tray.confDir` | Configuration Directory | client/src-tauri/src/core/tray/menu_def.rs:42 |
| `backend:tray.coreDir` | Core Directory | client/src-tauri/src/core/tray/menu_def.rs:43 |
| `backend:tray.logsDir` | Log Directory | client/src-tauri/src/core/tray/menu_def.rs:44 |
| `backend:tray.openDir` | Open Directory | client/src-tauri/src/core/tray/menu_def.rs:45 |
| `backend:tray.appLog` | Application Log | client/src-tauri/src/core/tray/menu_def.rs:46 |
| `backend:tray.coreLog` | Core Log | client/src-tauri/src/core/tray/menu_def.rs:47 |
| `backend:tray.restartClash` | Restart Clash Core | client/src-tauri/src/core/tray/menu_def.rs:48 |
| `backend:tray.restartApp` | Restart Application | client/src-tauri/src/core/tray/menu_def.rs:49 |
| `backend:tray.vergeVersion` | Verge Version | client/src-tauri/src/core/tray/menu_def.rs:50 |
| `backend:tray.more` | More | client/src-tauri/src/core/tray/menu_def.rs:51 |
| `backend:tray.exit` | Exit | client/src-tauri/src/core/tray/menu_def.rs:52 |
| `backend:tray.tooltip.systemProxy` | System Proxy | client/src-tauri/src/core/tray/mod.rs:320 |
| `backend:tray.tooltip.tun` | TUN | client/src-tauri/src/core/tray/mod.rs:321 |
| `backend:tray.tooltip.profile` | Profile | client/src-tauri/src/core/tray/mod.rs:322 |

## Deliberately not in the manifest

Strings that exist in the bundles but that a student cannot reach. Each is
listed with the reason, so "why is this not editable here?" has an answer.

| Key prefix | Why it is out of scope |
|---|---|
| `profiles.*` (10 strings) | The profiles UI was removed in 3.1.0. The keys stay because `notice-service.ts` maps them from Rust `PROFILE_*` error codes, which `cmd/profile.rs` still emits — deleting them would break a live toast. Not reachable from a screen. |
| `shared.actions.*` (5 strings) | The `components/base/` control set (dialog, search box, chip editor, text field, loading, switch, fieldset) is not rendered by any screen — the two-tab UI builds its own. The keys stay for now; deleting them is a separate change that should also delete the components. |
| `shared.editorModes.*` (2 strings) | Only the chip editor above uses it; unrendered. |
| `shared.labels.*` (1 strings) | `retryAttempts` is read by the traffic error boundary, which is reachable — but the label is a bare technical noun with no unit and no context, and it is the one string in the app that reads as an engine detail rather than a sentence. Listed here so a copy pass sees it; fix the row in `traffic-error-boundary.tsx` rather than the bundle if you want it gone. |
| `shared.placeholders.*` (5 strings) | Only the search box and text field above use these; both unrendered. |
| `shared.statuses.*` (1 strings) | Only the empty-state and chip editor above use it; both unrendered. |
| `shared.validation.*` (1 strings) | Only the search box above uses it; unrendered. |

## Not English-only: what a rewrite does and does not reach

`src/locales/en/` is the canonical bundle, but the app ships in 13
languages, and each of the other 12 owns the same key set. A rewrite of a
row in this file therefore has **four** possible outcomes, and only the
first is a finished job:

| The other locale's value | What `prose:apply` does | What a student sees |
|---|---|---|
| a real translation | leaves it alone | the **old** meaning, in their language |
| a copy of the old English | leaves it alone | the **old** English |
| the key is absent | leaves it absent | English, by fallback |
| a value the app cannot read | leaves it alone | nothing — the build fails |

That last row is not hypothetical and is the one here that is not merely
stale. The **native** strings are read by rust_i18n at COMPILE time, and a
lint-free YAML file can still fail it: a malformed `clash-verge-i18n/
locales/*.yml` stops `cargo build` with `proc macro panicked: Parse file …
failed`, on whichever platform builds it. `prose:apply` writes English only,
so a native row you rewrite leaves the other twelve untouched and valid —
but if you touch them by hand, run `cargo build -p clash-verge-i18n`.

So `prose:apply` makes the change true for `en` and for any key still
falling back — and **not** for a string someone translated. That is not a
bug to fix by copying English over the top: it would replace a real
translation with English. The row is finished when the translated value is
updated in that locale, which is a translator's job and is reviewed per
language.

`pnpm i18n:check` reports the drift: a key present in `en` and absent
elsewhere is `missing`, and a value identical to English in a locale that
normally translates is not distinguishable from a fallback copy — which is
why the count below is a floor, not a total.

Changing a placeholder needs the value re-checked in every language, and in
the right syntax: `{{used}}` in a JSON bundle (i18next), `{mode}` in
`en.yml` (rust_i18n). A count-sensitive string — `{{count}} days` — needs
its plural forms re-checked too: Japanese has one, Russian three, and
i18next will not warn when a rewrite changes which case a number falls into.
