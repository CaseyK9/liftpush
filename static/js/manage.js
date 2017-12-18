function activateItem(item) {
    item.addClass("active");

    var name = item.attr("data-name").split(".")[0];

    $("#file-src").css("background-image", "url(\"" + name + "\")");
    $("#file-name").text(item.attr("data-name"));
    $("#file-modal-name").text(item.attr("data-name"));
    $("#file-meta").text(item.attr("data-upload-name") + ", type: " + item.attr("data-type"));
    $("#file-open").attr("href", name);
}

function deleteFile() {
    var item = $(".collection-item.active");
    var name = item.attr("data-name").split(".")[0];
    console.log("Deleting " + name);

    $.ajax({
        "url": "delete/" + name
    });

    item.remove();

    $(".collection-item").removeClass("active");
    resetSelection();
}

function collectionClick() {
    $(".collection-item").removeClass("active");
    activateItem($(this));
}

function resetSelection() {
    var selector = $(".collection-item");
    selector.click(collectionClick);
    activateItem(selector.first());
}

$(function() {
    $("#delete-file-button").click(deleteFile);
    $('.modal').modal();

    resetSelection();
});
