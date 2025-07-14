# CPP_GUIDE.md

This file provides guidance for AI agents working with the C++/Qt version of the DiffLoupe application.

## Important Note on Build and Deployment Process (macOS)

The C++/Qt version of this application has a specific build and deployment process on macOS that requires manual intervention. **Do not rely on automated build commands within the IDE or standard `cmake && make` workflows alone.**

The correct procedure is as follows:

1.  **Clean and Build**:
    ```bash
    rm -rf build/* && cmake -B build && make -C build
    ```
2.  **Deploy with `macdeployqt`**:
    After a successful build, you **must** run `macdeployqt` from within the `build` directory to correctly bundle the Qt frameworks and dependencies.
    ```bash
    cd build && macdeployqt DiffLoupe.app
    ```
    If you encounter issues, you may need to specify the executable path:
    ```bash
    cd build && macdeployqt DiffLoupe.app -executable=DiffLoupe.app/Contents/MacOS/DiffLoupe
    ```

**Failure to follow this manual deployment step will result in a non-functional application bundle that will crash on launch due to missing libraries.**
