# Install script for directory: C:/tess/third_party/tesseract

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "C:/tess/tesseract")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE RENAME "tesseract.pc" FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/tesseract_Debug.pc")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE RENAME "tesseract.pc" FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/tesseract_Release.pc")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE RENAME "tesseract.pc" FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/tesseract_MinSizeRel.pc")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE RENAME "tesseract.pc" FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/tesseract_RelWithDebInfo.pc")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE EXECUTABLE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/Debug/tesseract.exe")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE EXECUTABLE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/Release/tesseract.exe")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE EXECUTABLE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/MinSizeRel/tesseract.exe")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE EXECUTABLE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/RelWithDebInfo/tesseract.exe")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    include("E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/tesseract.dir/install-cxx-module-bmi-Debug.cmake" OPTIONAL)
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    include("E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/tesseract.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    include("E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/tesseract.dir/install-cxx-module-bmi-MinSizeRel.cmake" OPTIONAL)
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    include("E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/tesseract.dir/install-cxx-module-bmi-RelWithDebInfo.cmake" OPTIONAL)
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE FILE OPTIONAL FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/Debug/tesseract.pdb")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE FILE OPTIONAL FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/Release/tesseract.pdb")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE FILE OPTIONAL FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/MinSizeRel/tesseract.pdb")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/bin" TYPE FILE OPTIONAL FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/bin/RelWithDebInfo/tesseract.pdb")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/Debug/tesseract55d.lib")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/Release/tesseract55.lib")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/MinSizeRel/tesseract55.lib")
  elseif(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/RelWithDebInfo/tesseract55.lib")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract/TesseractTargets.cmake")
    file(DIFFERENT _cmake_export_file_changed FILES
         "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract/TesseractTargets.cmake"
         "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets.cmake")
    if(_cmake_export_file_changed)
      file(GLOB _cmake_old_config_files "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract/TesseractTargets-*.cmake")
      if(_cmake_old_config_files)
        string(REPLACE ";" ", " _cmake_old_config_files_text "${_cmake_old_config_files}")
        message(STATUS "Old export file \"$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract/TesseractTargets.cmake\" will be replaced.  Removing files [${_cmake_old_config_files_text}].")
        unset(_cmake_old_config_files_text)
        file(REMOVE ${_cmake_old_config_files})
      endif()
      unset(_cmake_old_config_files)
    endif()
    unset(_cmake_export_file_changed)
  endif()
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract" TYPE FILE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets.cmake")
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Dd][Ee][Bb][Uu][Gg])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract" TYPE FILE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets-debug.cmake")
  endif()
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Mm][Ii][Nn][Ss][Ii][Zz][Ee][Rr][Ee][Ll])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract" TYPE FILE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets-minsizerel.cmake")
  endif()
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ww][Ii][Tt][Hh][Dd][Ee][Bb][Ii][Nn][Ff][Oo])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract" TYPE FILE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets-relwithdebinfo.cmake")
  endif()
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/tesseract" TYPE FILE FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/CMakeFiles/Export/bccb9fa002ffd7d7c10327ccec7a25bf/TesseractTargets-release.cmake")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE DIRECTORY FILES "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/cmake")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/tesseract" TYPE FILE FILES
    "C:/tess/third_party/tesseract/include/tesseract/baseapi.h"
    "C:/tess/third_party/tesseract/include/tesseract/capi.h"
    "C:/tess/third_party/tesseract/include/tesseract/renderer.h"
    "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/include/tesseract/version.h"
    "C:/tess/third_party/tesseract/include/tesseract/ltrresultiterator.h"
    "C:/tess/third_party/tesseract/include/tesseract/pageiterator.h"
    "C:/tess/third_party/tesseract/include/tesseract/resultiterator.h"
    "C:/tess/third_party/tesseract/include/tesseract/osdetect.h"
    "C:/tess/third_party/tesseract/include/tesseract/publictypes.h"
    "C:/tess/third_party/tesseract/include/tesseract/ocrclass.h"
    "C:/tess/third_party/tesseract/include/tesseract/export.h"
    "C:/tess/third_party/tesseract/include/tesseract/unichar.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/share/tessdata/configs" TYPE FILE FILES
    "C:/tess/third_party/tesseract/tessdata/configs/alto"
    "C:/tess/third_party/tesseract/tessdata/configs/ambigs.train"
    "C:/tess/third_party/tesseract/tessdata/configs/api_config"
    "C:/tess/third_party/tesseract/tessdata/configs/bazaar"
    "C:/tess/third_party/tesseract/tessdata/configs/bigram"
    "C:/tess/third_party/tesseract/tessdata/configs/box.train"
    "C:/tess/third_party/tesseract/tessdata/configs/box.train.stderr"
    "C:/tess/third_party/tesseract/tessdata/configs/digits"
    "C:/tess/third_party/tesseract/tessdata/configs/get.images"
    "C:/tess/third_party/tesseract/tessdata/configs/hocr"
    "C:/tess/third_party/tesseract/tessdata/configs/inter"
    "C:/tess/third_party/tesseract/tessdata/configs/kannada"
    "C:/tess/third_party/tesseract/tessdata/configs/linebox"
    "C:/tess/third_party/tesseract/tessdata/configs/logfile"
    "C:/tess/third_party/tesseract/tessdata/configs/lstm.train"
    "C:/tess/third_party/tesseract/tessdata/configs/lstmbox"
    "C:/tess/third_party/tesseract/tessdata/configs/lstmdebug"
    "C:/tess/third_party/tesseract/tessdata/configs/makebox"
    "C:/tess/third_party/tesseract/tessdata/configs/page"
    "C:/tess/third_party/tesseract/tessdata/configs/pdf"
    "C:/tess/third_party/tesseract/tessdata/configs/quiet"
    "C:/tess/third_party/tesseract/tessdata/configs/rebox"
    "C:/tess/third_party/tesseract/tessdata/configs/strokewidth"
    "C:/tess/third_party/tesseract/tessdata/configs/tsv"
    "C:/tess/third_party/tesseract/tessdata/configs/txt"
    "C:/tess/third_party/tesseract/tessdata/configs/unlv"
    "C:/tess/third_party/tesseract/tessdata/configs/wordstrbox"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/share/tessdata/tessconfigs" TYPE FILE FILES
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/batch"
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/batch.nochop"
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/matdemo"
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/msdemo"
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/nobatch"
    "C:/tess/third_party/tesseract/tessdata/tessconfigs/segdemo"
    )
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
if(CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/install_local_manifest.txt"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
if(CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_COMPONENT MATCHES "^[a-zA-Z0-9_.+-]+$")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INSTALL_COMPONENT}.txt")
  else()
    string(MD5 CMAKE_INST_COMP_HASH "${CMAKE_INSTALL_COMPONENT}")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INST_COMP_HASH}.txt")
    unset(CMAKE_INST_COMP_HASH)
  endif()
else()
  set(CMAKE_INSTALL_MANIFEST "install_manifest.txt")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "E:/findall/target_local/debug/build/kreuzberg-tesseract-acaa429d286aad54/out/build/${CMAKE_INSTALL_MANIFEST}"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
