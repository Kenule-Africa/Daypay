const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);
const button = document.getElementById("verify");
const feedback = document.getElementById("status");
if (token) button.disabled = false;
else feedback.textContent = "Open the verification link from your email.";
button.addEventListener("click", async () => {
    button.disabled = true;
    try {
        const response = await fetch("/api/v1/auth/verify-email", {
            method: "POST", credentials: "omit", cache: "no-store", referrerPolicy: "no-referrer",
            headers: {"Content-Type": "application/json"}, body: JSON.stringify({token})
        });
        if (response.ok) feedback.textContent = "Your email address is verified.";
        else if (response.status === 422) feedback.textContent = "This link is invalid or expired. Request a new verification email.";
        else throw new Error();
    } catch {
        feedback.textContent = "Verification is temporarily unavailable. Try again later.";
        button.disabled = false;
    }
});
