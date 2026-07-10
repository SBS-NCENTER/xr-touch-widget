// Tag <html> with the host OS so CSS can special-case it.
//
// On Windows the widget runs in an OPAQUE window: a transparent WebView2 window
// does not composite its content on Windows 11 (palette + settings rendered
// blank — content was live and interactive but never painted). The window is
// forced opaque there (tauri.windows.conf.json + open_settings cfg), and the
// glass surfaces switch to solid backgrounds via the `.platform-windows` class.
// macOS/Linux are unaffected (WKWebView composites transparency fine).
if (navigator.userAgent.includes('Windows')) {
  document.documentElement.classList.add('platform-windows');
}
