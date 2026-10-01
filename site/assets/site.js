document.querySelectorAll('[data-copy]').forEach((button) => {
  button.addEventListener('click', async () => {
    const code = button.parentElement.querySelector('code');
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.textContent);
      button.textContent = 'Copied';
    } catch {
      button.textContent = 'Select and copy';
    }
    window.setTimeout(() => { button.textContent = 'Copy'; }, 1200);
  });
});
