console.log("Example Plugin Loaded!");

// Example: Add a button to the toolbar (hypothetical API)
if (window.app && window.app.toolbar) {
    window.app.toolbar.addButton({
        icon: "star",
        label: "Hello",
        onClick: () => alert("Hello from Plugin!")
    });
}
