const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);
const form = document.getElementById("reset");
const password = document.getElementById("password");
const confirmation = document.getElementById("confirm");
const submit = document.getElementById("submit");
const feedback = document.getElementById("status");
const controls = [password, confirmation, submit];
const enable = on => controls.forEach(control => { control.disabled = !on; });
if (token) enable(true);
else feedback.textContent = "Open the password reset link from your email.";
form.addEventListener("submit", async event => {
    event.preventDefault();
    if (!password.value) {
        feedback.textContent = "Enter a new password.";
        return;
    }
    if (password.value !== confirmation.value) {
        feedback.textContent = "The passwords do not match.";
        return;
    }
    enable(false);
    let retry = true;
    try {
        const response = await fetch("/api/v1/auth/reset-password", {
            method: "POST", credentials: "omit", cache: "no-store", referrerPolicy: "no-referrer",
            headers: {"Content-Type": "application/json"},
            body: JSON.stringify({token, password: password.value})
        });
        if (response.ok) {
            retry = false;
            feedback.textContent = "Your password has been reset. Sign in with your new password.";
        } else if (response.status === 422) {
            const body = await response.json().catch(() => ({}));
            if (body?.error?.details?.reason === "password_rejected") {
                feedback.textContent = "That password does not meet the password requirements or is known to be compromised. Choose a different password.";
            } else {
                retry = false;
                feedback.textContent = "This link is invalid, expired or already used. Request a new password reset email.";
            }
        } else if (response.status === 429) {
            feedback.textContent = "Too many attempts. Wait a minute and try again.";
        } else {
            throw new Error();
        }
    } catch {
        feedback.textContent = "Password reset is temporarily unavailable. Try again later.";
    }
    password.value = "";
    confirmation.value = "";
    if (retry) enable(true);
});
