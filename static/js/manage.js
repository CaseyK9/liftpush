function activateItem(item) {
    item.addClass("active");

    var type = item.attr("data-type");
    var name;
    if (type === "url") {
        name = item.attr("data-url");
        $("#file-src").css("display", "none");
        $("#file-name").text(name);
        $("#file-modal-name").text(name);
        $("#rename-modal-name").text(name);
        $("#file-meta").html("Redirect to <a href=\"" +
            item.attr("data-target") + "\" target='_blank'>" + item.attr("data-target") + "</a>");
        $("#file-open").attr("href", name);
    } else {
        name = item.attr("data-url");
        var src = $("#file-src");
        src.css("display", "block");
        src.css("background-image", "url(\"" + name + "\")");
        $("#file-name").text(item.attr("data-name"));
        $("#file-modal-name").text(item.attr("data-name"));
        $("#rename-modal-name").text(item.attr("data-name"));
        $("#file-meta").text(item.attr("data-upload-name") + ", type: " + type);
        $("#file-open").attr("href", name);
    }
}

function deleteFile() {
    var item = $(".collection-item.active");
    var name = item.attr("data-url");
    console.log("Deleting " + name);

    $.ajax({
        "url": "delete/" + name
    });

    item.remove();

    $(".collection-item").removeClass("active");
    resetSelection();
}

function renameFile() {
    var item = $(".collection-item.active");
    var name = item.attr("data-url");
    var target = $("#rename-modal-target").val();
    console.log("Renaming " + name + " to " + target);

    $.ajax({
        "url": "rename/" + name + "/" +  target
    }).done(function() {
        document.location = document.location + "?";
    });

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
    $("#rename-file-button").click(renameFile);
    $('.modal').modal();

    resetSelection();
});
