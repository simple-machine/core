#include <unistd.h>
#include <stdio.h>
#include <string.h>
#include "target/core.h"

int main() {
    smov_sender_t* sender;
    smov_handle_t* handle;
    char *text = "/dev/ttyACM2";
	smov_error_t err = smov_connect(text, &sender, &handle);
	if (err.tag != OK) {
    	printf("fatal error: %s\n", smov_strerror(err));
    	if (err.tag == COMMUNICATION) {
        	printf("  => %s\n", strerror(err.payload));
    	} else if (err.tag == UNSUPPORTED_VERSION) {
        	printf("  => version was %d\n", err.payload);
    	}
    	return 1;
	}
	
	while (smov_set_speed(sender, -23)) {
    	sleep(1);
	}
	err = smov_get_error(handle);
	if (err.tag != OK) {
    	smov_drop_communication(sender);
    	printf("fatal error: %s\n", smov_strerror(err));
    	return 1;
	}
	smov_drop_communication(sender);
}
