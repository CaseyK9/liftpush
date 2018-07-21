/**
 * Makes an AJAX request, returning a JSON payload. Async.
 */
function ajaxRequest(url, asJson = false) {
    return new Promise(function (resolve, reject) {
        const xmlRequest = new XMLHttpRequest();
        xmlRequest.onload = function() {
            if (xmlRequest.status >= 200 && xmlRequest.status < 300) {
                if (asJson) {
                    resolve(JSON.parse(xmlRequest.response));
                } else {
                    resolve(xmlRequest.response);
                }
            } else {
                console.error("Bad response: " + xmlRequest.response);
                reject(Error(xmlRequest.statusText));
            }
        };

        xmlRequest.open("GET", url);

        xmlRequest.send();
    });
}

// https://stackoverflow.com/questions/5767325/how-do-i-remove-a-particular-element-from-an-array-in-javascript
function remove(arr, item) {
    for (let i = arr.length; i--;) {
        if (arr[i] === item) {
            arr.splice(i, 1);
        }
    }
}

const app = new Vue({
    el: '#index-banner',
    data: {
        items: [],
        active_item: undefined,
        rename_value: ""
    },
    methods: {
        deleteFile: function(event) {
            const name = app.active_item.name;
            console.log("Deleting " + name);

            ajaxRequest("delete/" + name).then(function() {
                console.log("Delete OK")
            });

            remove(app.items, app.active_item);

            app.active_item = undefined;
        },
        renameFile: function(event) {
            const name = app.active_item.name;
            const target = app.rename_value;

            app.rename_value = "";

            console.log("Renaming " + name + " to " + target);

            ajaxRequest("rename/" + name + "/" + target).then(function() {
                console.log("Rename OK")
            });

            let new_active_item = app.active_item;
            new_active_item.name = target;
            if (new_active_item.meta.actual_filename) {
                const extension = new_active_item.meta.actual_filename.split(".")[1];
                new_active_item.meta.actual_filename = target + "." + extension;
            }

            for (let i = app.items.length; i--;) {
                if (app.items[i] === app.active_item) {
                    app.items[i] = new_active_item;
                }
            }

            app.active_item = new_active_item;
        }
    }
});

/**
 * Updates the Vue set of items from the server.
 */
async function updateListing() {
    app.items = (await ajaxRequest("/listing", true)).files;
}

window.addEventListener("DOMContentLoaded", function () {
    updateListing().then(function() {});
});
