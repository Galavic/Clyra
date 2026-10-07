export default {
  fetch(request) {
    const url = new URL(request.url);
    url.hostname = "clyra-cli.xyz";
    return Response.redirect(url.toString(), 301);
  },
};
