# ba | Next Generation Image
A Graphical User Interface (GUI) for the conversion tools of most popular next-generation open-source image formats. Including WebP and JPG2000(coming soon).

This project is an electron app that simply generates a shell command and passes that command to the binaries provided by the open-source projects (for example cwebp.exe).

# Screenshot
![screenshot](https://basicautonomy.com/ba-nextGenIMG/images/screenshots/nextgenimg-v0-1-1.webp)


## Minimum Path to Awesome

**Clone git repository: **

`git clone git@github.com:Creative515/ba-nextGenIMG.git`

**Install yarn:**

`npm install -g yarn`

**Install dependencies:**

`yarn install`

**Create directory:**

`./resources/win/bin/`

**Download WebP CLI:**

`https://developers.google.com/speed/webp/docs/precompiled`
and place cwebp.exe (found in ./bin/cwebp.exe) in 
`./resources/win/bin/`

**Create distributable**

`yarn dist`

**Load program**

Double click: `./dist/bin/ba-nextgenimg 0.0.1.exe`
