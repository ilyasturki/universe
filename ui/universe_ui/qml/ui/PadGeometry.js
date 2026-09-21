.pragma library

// Each family's pad on a 1000 × 720 sheet: the DualSense traced from a front photograph, the Xbox from Microsoft's render.

var VIEW = [1000, 720];

var BODIES = {
    dualsense: "M 466 100 L 444 101 L 422 101 L 402 102 L 382 102 L 363 103 L 344 104 L 326 105 L 307 107 L 288 109 L 270 111 L 252 114 L 235 117 L 219 120 L 205 123 L 195 126 L 188 128 L 184 130 L 180 132 L 177 135 L 174 137 L 170 140 L 166 145 L 161 151 L 154 160 L 145 171 L 138 182 L 131 193 L 126 205 L 120 216 L 115 230 L 110 246 L 104 264 L 99 284 L 93 303 L 89 322 L 84 340 L 81 357 L 77 374 L 74 392 L 71 409 L 69 426 L 67 445 L 65 467 L 63 490 L 62 515 L 62 538 L 62 559 L 63 577 L 66 594 L 68 607 L 70 617 L 71 624 L 73 628 L 74 632 L 76 636 L 79 640 L 81 643 L 84 647 L 88 651 L 93 655 L 99 659 L 107 664 L 119 670 L 133 676 L 150 684 L 164 689 L 174 691 L 180 691 L 184 688 L 187 684 L 191 678 L 195 671 L 199 663 L 207 646 L 217 620 L 230 587 L 246 544 L 259 512 L 269 489 L 275 475 L 279 471 L 283 467 L 288 464 L 293 460 L 299 458 L 305 455 L 311 453 L 317 451 L 322 450 L 337 449 L 361 449 L 394 450 L 437 450 L 479 451 L 521 451 L 563 450 L 606 450 L 639 449 L 663 449 L 678 450 L 683 451 L 689 453 L 695 455 L 701 458 L 707 460 L 712 464 L 717 467 L 721 471 L 725 475 L 731 489 L 741 512 L 754 544 L 770 587 L 783 620 L 793 646 L 801 663 L 805 671 L 809 678 L 813 684 L 816 688 L 820 691 L 826 691 L 836 689 L 850 684 L 867 676 L 881 670 L 893 664 L 901 659 L 907 655 L 912 651 L 916 647 L 919 643 L 921 640 L 924 636 L 926 632 L 927 628 L 929 624 L 930 617 L 932 607 L 934 594 L 937 577 L 938 559 L 938 538 L 938 515 L 937 490 L 935 467 L 933 445 L 931 426 L 929 409 L 926 392 L 923 374 L 919 357 L 916 340 L 911 322 L 907 303 L 901 284 L 896 264 L 890 246 L 885 230 L 880 216 L 874 205 L 869 193 L 862 182 L 855 171 L 846 160 L 839 151 L 834 145 L 830 140 L 826 137 L 823 135 L 820 132 L 816 130 L 812 128 L 805 126 L 795 123 L 781 120 L 765 117 L 748 114 L 730 111 L 712 109 L 693 107 L 674 105 L 656 104 L 637 103 L 618 102 L 598 102 L 578 101 L 556 101 L 534 100 L 511 100 L 489 100 Z",
    xbox: "M 473 100 L 419 101 L 378 102 L 349 105 L 315 110 L 275 119 L 244 126 L 224 134 L 208 139 L 200 142 L 190 151 L 182 165 L 172 182 L 164 203 L 150 236 L 133 283 L 117 326 L 104 367 L 92 402 L 84 432 L 76 459 L 71 483 L 66 506 L 64 528 L 62 548 L 60 564 L 60 580 L 62 592 L 63 604 L 66 615 L 69 626 L 74 636 L 79 648 L 86 659 L 92 669 L 97 676 L 105 682 L 115 688 L 126 691 L 139 692 L 150 692 L 160 689 L 169 685 L 176 678 L 184 671 L 191 664 L 198 656 L 206 649 L 224 627 L 253 592 L 274 568 L 284 558 L 295 548 L 305 542 L 315 536 L 325 532 L 337 529 L 352 526 L 394 525 L 465 525 L 535 525 L 606 525 L 648 526 L 663 529 L 675 532 L 685 536 L 695 542 L 705 548 L 716 558 L 726 568 L 747 592 L 776 627 L 794 649 L 802 656 L 809 664 L 816 671 L 824 678 L 831 685 L 840 689 L 850 692 L 861 692 L 874 691 L 885 688 L 895 682 L 903 676 L 908 669 L 914 659 L 921 648 L 926 636 L 931 626 L 934 615 L 937 604 L 938 592 L 940 580 L 940 564 L 938 548 L 936 528 L 934 506 L 929 483 L 924 459 L 916 432 L 908 402 L 896 367 L 883 326 L 867 283 L 850 236 L 836 203 L 828 182 L 818 165 L 810 151 L 800 142 L 792 139 L 776 134 L 756 126 L 725 119 L 685 110 L 651 105 L 622 102 L 581 101 L 527 100 Z",
    generic: "M 500 106 L 530 106 L 560 107 L 589 108 L 618 109 L 646 111 L 673 114 L 699 117 L 724 121 L 747 125 L 768 130 L 791 137 L 811 146 L 830 157 L 848 170 L 863 185 L 877 201 L 889 219 L 898 238 L 906 258 L 912 280 L 916 300 L 919 321 L 922 343 L 924 365 L 926 388 L 926 410 L 927 434 L 926 457 L 926 481 L 924 504 L 922 526 L 918 548 L 913 568 L 906 586 L 898 603 L 889 617 L 878 630 L 865 640 L 851 647 L 836 652 L 823 653 L 810 652 L 799 648 L 789 642 L 779 634 L 770 625 L 762 614 L 755 602 L 748 590 L 742 576 L 735 562 L 728 549 L 721 537 L 713 526 L 704 517 L 695 509 L 684 503 L 673 497 L 661 493 L 648 490 L 633 488 L 619 487 L 604 486 L 589 485 L 574 485 L 559 485 L 544 485 L 529 486 L 515 486 L 500 486 L 485 486 L 471 486 L 456 485 L 441 485 L 426 485 L 411 485 L 396 486 L 381 487 L 367 488 L 352 490 L 339 493 L 327 497 L 316 503 L 305 509 L 296 517 L 287 526 L 279 537 L 272 549 L 265 562 L 258 576 L 252 590 L 245 602 L 238 614 L 230 625 L 221 634 L 211 642 L 201 648 L 190 652 L 177 653 L 164 652 L 149 647 L 135 640 L 122 630 L 111 617 L 102 603 L 94 586 L 87 568 L 82 548 L 78 526 L 76 504 L 75 481 L 74 457 L 73 434 L 74 410 L 74 388 L 76 365 L 78 343 L 81 321 L 84 300 L 88 280 L 94 258 L 102 238 L 111 219 L 123 201 L 137 185 L 152 170 L 170 157 L 189 146 L 209 137 L 232 130 L 253 125 L 276 121 L 301 117 L 327 114 L 354 111 L 382 109 L 411 108 L 440 107 L 470 106 L 500 106 Z"
};

var SHOULDERS = {
    dualsense: {
        lb: {path: "M 154 173 L 150 169 L 149 163 L 150 158 L 154 154 L 160 152 L 164 147 L 168 143 L 172 138 L 176 135 L 180 132 L 184 130 L 188 128 L 192 127 L 196 125 L 200 124 L 204 123 L 208 122 L 212 122 L 216 121 L 220 120 L 224 119 L 228 118 L 232 118 L 236 117 L 240 116 L 244 115 L 248 115 L 252 114 L 256 113 L 260 113 L 264 112 L 268 112 L 272 111 L 276 111 L 280 110 L 284 110 L 288 109 L 292 109 L 296 108 L 300 108 L 306 109 L 310 113 L 311 119 L 310 124 L 306 128 L 300 130 L 296 130 L 292 131 L 288 131 L 284 132 L 280 132 L 276 133 L 272 133 L 268 134 L 264 134 L 260 135 L 256 135 L 252 136 L 248 137 L 244 137 L 240 138 L 236 139 L 232 140 L 228 140 L 224 141 L 220 142 L 216 143 L 212 144 L 208 144 L 204 145 L 200 146 L 196 147 L 192 149 L 188 150 L 184 152 L 180 154 L 176 157 L 172 160 L 168 165 L 164 169 L 160 174 Z", b: [149, 108, 311, 174]},
        lt: {path: "M 204 97 L 206 90 L 208 87 L 210 85 L 212 83 L 214 82 L 216 81 L 218 80 L 220 80 L 222 79 L 224 79 L 226 79 L 228 78 L 230 78 L 232 78 L 234 77 L 236 77 L 238 76 L 240 76 L 242 76 L 244 75 L 246 75 L 248 75 L 250 74 L 252 74 L 254 74 L 256 73 L 258 73 L 260 73 L 262 72 L 264 72 L 266 72 L 268 72 L 270 71 L 272 71 L 274 71 L 276 71 L 278 70 L 280 71 L 282 71 L 284 72 L 286 73 L 288 76 L 290 83 L 290 113 L 288 113 L 286 113 L 284 114 L 282 114 L 280 114 L 278 114 L 276 115 L 274 115 L 272 115 L 270 115 L 268 116 L 266 116 L 264 116 L 262 116 L 260 117 L 258 117 L 256 117 L 254 118 L 252 118 L 250 118 L 248 119 L 246 119 L 244 119 L 242 120 L 240 120 L 238 120 L 236 121 L 234 121 L 232 122 L 230 122 L 228 122 L 226 123 L 224 123 L 222 123 L 220 124 L 218 124 L 216 125 L 214 125 L 212 126 L 210 126 L 208 126 L 206 127 L 204 127 Z", b: [204, 70, 290, 127]},
        rb: {path: "M 846 173 L 850 169 L 851 163 L 850 158 L 846 154 L 840 152 L 836 147 L 832 143 L 828 138 L 824 135 L 820 132 L 816 130 L 812 128 L 808 127 L 804 125 L 800 124 L 796 123 L 792 122 L 788 122 L 784 121 L 780 120 L 776 119 L 772 118 L 768 118 L 764 117 L 760 116 L 756 115 L 752 115 L 748 114 L 744 113 L 740 113 L 736 112 L 732 112 L 728 111 L 724 111 L 720 110 L 716 110 L 712 109 L 708 109 L 704 108 L 700 108 L 694 109 L 690 113 L 689 119 L 690 124 L 694 128 L 700 130 L 704 130 L 708 131 L 712 131 L 716 132 L 720 132 L 724 133 L 728 133 L 732 134 L 736 134 L 740 135 L 744 135 L 748 136 L 752 137 L 756 137 L 760 138 L 764 139 L 768 140 L 772 140 L 776 141 L 780 142 L 784 143 L 788 144 L 792 144 L 796 145 L 800 146 L 804 147 L 808 149 L 812 150 L 816 152 L 820 154 L 824 157 L 828 160 L 832 165 L 836 169 L 840 174 Z", b: [689, 108, 851, 174]},
        rt: {path: "M 796 97 L 794 90 L 792 87 L 790 85 L 788 83 L 786 82 L 784 81 L 782 80 L 780 80 L 778 79 L 776 79 L 774 79 L 772 78 L 770 78 L 768 78 L 766 77 L 764 77 L 762 76 L 760 76 L 758 76 L 756 75 L 754 75 L 752 75 L 750 74 L 748 74 L 746 74 L 744 73 L 742 73 L 740 73 L 738 72 L 736 72 L 734 72 L 732 72 L 730 71 L 728 71 L 726 71 L 724 71 L 722 70 L 720 71 L 718 71 L 716 72 L 714 73 L 712 76 L 710 83 L 710 113 L 712 113 L 714 113 L 716 114 L 718 114 L 720 114 L 722 114 L 724 115 L 726 115 L 728 115 L 730 115 L 732 116 L 734 116 L 736 116 L 738 116 L 740 117 L 742 117 L 744 117 L 746 118 L 748 118 L 750 118 L 752 119 L 754 119 L 756 119 L 758 120 L 760 120 L 762 120 L 764 121 L 766 121 L 768 122 L 770 122 L 772 122 L 774 123 L 776 123 L 778 123 L 780 124 L 782 124 L 784 125 L 786 125 L 788 126 L 790 126 L 792 126 L 794 127 L 796 127 Z", b: [710, 70, 796, 127]}
    },
    xbox: {
        lb: {path: "M 206 160 L 202 156 L 200 150 L 202 144 L 206 139 L 212 138 L 216 136 L 220 135 L 224 133 L 228 132 L 232 131 L 236 129 L 240 128 L 244 127 L 248 126 L 252 125 L 256 124 L 260 123 L 264 122 L 268 121 L 272 120 L 276 119 L 280 118 L 284 117 L 288 116 L 292 115 L 296 114 L 300 113 L 304 113 L 308 112 L 312 111 L 316 110 L 320 109 L 324 109 L 328 108 L 332 108 L 336 107 L 340 106 L 344 106 L 348 105 L 352 105 L 356 104 L 360 104 L 364 103 L 368 103 L 372 103 L 378 104 L 382 109 L 384 115 L 382 121 L 378 125 L 372 127 L 368 127 L 364 127 L 360 128 L 356 128 L 352 129 L 348 129 L 344 130 L 340 130 L 336 131 L 332 132 L 328 132 L 324 133 L 320 133 L 316 134 L 312 135 L 308 136 L 304 137 L 300 137 L 296 138 L 292 139 L 288 140 L 284 141 L 280 142 L 276 143 L 272 144 L 268 145 L 264 146 L 260 147 L 256 148 L 252 149 L 248 150 L 244 151 L 240 152 L 236 153 L 232 155 L 228 156 L 224 157 L 220 159 L 216 160 L 212 162 Z", b: [200, 103, 384, 162]},
        lt: {path: "M 214 111 L 216 103 L 218 100 L 220 97 L 222 95 L 224 94 L 226 93 L 228 92 L 230 91 L 232 91 L 234 90 L 236 89 L 238 89 L 240 88 L 242 87 L 244 87 L 246 86 L 248 86 L 250 85 L 252 85 L 254 84 L 256 84 L 258 83 L 260 83 L 262 82 L 264 82 L 266 81 L 268 81 L 270 80 L 272 80 L 274 79 L 276 79 L 278 78 L 280 78 L 282 77 L 284 77 L 286 76 L 288 76 L 290 76 L 292 75 L 294 75 L 296 74 L 298 74 L 300 73 L 302 73 L 304 73 L 306 72 L 308 72 L 310 71 L 312 71 L 314 70 L 316 70 L 318 70 L 320 69 L 322 69 L 324 69 L 326 68 L 328 68 L 330 68 L 332 68 L 334 68 L 336 68 L 338 69 L 340 70 L 342 73 L 344 80 L 344 110 L 342 110 L 340 110 L 338 111 L 336 111 L 334 111 L 332 112 L 330 112 L 328 112 L 326 112 L 324 113 L 322 113 L 320 113 L 318 114 L 316 114 L 314 114 L 312 115 L 310 115 L 308 116 L 306 116 L 304 117 L 302 117 L 300 117 L 298 118 L 296 118 L 294 119 L 292 119 L 290 120 L 288 120 L 286 120 L 284 121 L 282 121 L 280 122 L 278 122 L 276 123 L 274 123 L 272 124 L 270 124 L 268 125 L 266 125 L 264 126 L 262 126 L 260 127 L 258 127 L 256 128 L 254 128 L 252 129 L 250 129 L 248 130 L 246 130 L 244 131 L 242 131 L 240 132 L 238 133 L 236 133 L 234 134 L 232 135 L 230 135 L 228 136 L 226 137 L 224 137 L 222 138 L 220 139 L 218 139 L 216 140 L 214 141 Z", b: [214, 68, 344, 141]},
        rb: {path: "M 794 160 L 798 156 L 800 150 L 798 144 L 794 139 L 788 138 L 784 136 L 780 135 L 776 133 L 772 132 L 768 131 L 764 129 L 760 128 L 756 127 L 752 126 L 748 125 L 744 124 L 740 123 L 736 122 L 732 121 L 728 120 L 724 119 L 720 118 L 716 117 L 712 116 L 708 115 L 704 114 L 700 113 L 696 113 L 692 112 L 688 111 L 684 110 L 680 109 L 676 109 L 672 108 L 668 108 L 664 107 L 660 106 L 656 106 L 652 105 L 648 105 L 644 104 L 640 104 L 636 103 L 632 103 L 628 103 L 622 104 L 618 109 L 616 115 L 618 121 L 622 125 L 628 127 L 632 127 L 636 127 L 640 128 L 644 128 L 648 129 L 652 129 L 656 130 L 660 130 L 664 131 L 668 132 L 672 132 L 676 133 L 680 133 L 684 134 L 688 135 L 692 136 L 696 137 L 700 137 L 704 138 L 708 139 L 712 140 L 716 141 L 720 142 L 724 143 L 728 144 L 732 145 L 736 146 L 740 147 L 744 148 L 748 149 L 752 150 L 756 151 L 760 152 L 764 153 L 768 155 L 772 156 L 776 157 L 780 159 L 784 160 L 788 162 Z", b: [616, 103, 800, 162]},
        rt: {path: "M 786 111 L 784 103 L 782 100 L 780 97 L 778 95 L 776 94 L 774 93 L 772 92 L 770 91 L 768 91 L 766 90 L 764 89 L 762 89 L 760 88 L 758 87 L 756 87 L 754 86 L 752 86 L 750 85 L 748 85 L 746 84 L 744 84 L 742 83 L 740 83 L 738 82 L 736 82 L 734 81 L 732 81 L 730 80 L 728 80 L 726 79 L 724 79 L 722 78 L 720 78 L 718 77 L 716 77 L 714 76 L 712 76 L 710 76 L 708 75 L 706 75 L 704 74 L 702 74 L 700 73 L 698 73 L 696 73 L 694 72 L 692 72 L 690 71 L 688 71 L 686 70 L 684 70 L 682 70 L 680 69 L 678 69 L 676 69 L 674 68 L 672 68 L 670 68 L 668 68 L 666 68 L 664 68 L 662 69 L 660 70 L 658 73 L 656 80 L 656 110 L 658 110 L 660 110 L 662 111 L 664 111 L 666 111 L 668 112 L 670 112 L 672 112 L 674 112 L 676 113 L 678 113 L 680 113 L 682 114 L 684 114 L 686 114 L 688 115 L 690 115 L 692 116 L 694 116 L 696 117 L 698 117 L 700 117 L 702 118 L 704 118 L 706 119 L 708 119 L 710 120 L 712 120 L 714 120 L 716 121 L 718 121 L 720 122 L 722 122 L 724 123 L 726 123 L 728 124 L 730 124 L 732 125 L 734 125 L 736 126 L 738 126 L 740 127 L 742 127 L 744 128 L 746 128 L 748 129 L 750 129 L 752 130 L 754 130 L 756 131 L 758 131 L 760 132 L 762 133 L 764 133 L 766 134 L 768 135 L 770 135 L 772 136 L 774 137 L 776 137 L 778 138 L 780 139 L 782 139 L 784 140 L 786 141 Z", b: [656, 68, 786, 141]}
    },
    generic: {
        lb: {path: "M 190 163 L 186 159 L 185 154 L 186 148 L 190 144 L 196 143 L 200 141 L 204 139 L 208 138 L 212 136 L 216 135 L 220 134 L 224 133 L 228 131 L 232 130 L 236 129 L 240 128 L 244 127 L 248 126 L 252 125 L 256 124 L 260 124 L 264 123 L 268 122 L 272 121 L 276 121 L 280 120 L 284 119 L 288 119 L 292 118 L 296 118 L 300 117 L 304 117 L 308 116 L 312 116 L 316 115 L 320 115 L 324 114 L 328 114 L 332 113 L 336 113 L 344 114 L 348 118 L 349 124 L 348 129 L 344 133 L 336 135 L 332 135 L 328 136 L 324 136 L 320 137 L 316 137 L 312 138 L 308 138 L 304 139 L 300 139 L 296 140 L 292 140 L 288 141 L 284 141 L 280 142 L 276 143 L 272 143 L 268 144 L 264 145 L 260 146 L 256 146 L 252 147 L 248 148 L 244 149 L 240 150 L 236 151 L 232 152 L 228 153 L 224 155 L 220 156 L 216 157 L 212 158 L 208 160 L 204 161 L 200 163 L 196 165 Z", b: [185, 113, 349, 165]},
        lt: {path: "M 226 106 L 228 98 L 230 95 L 232 93 L 234 91 L 236 90 L 238 89 L 240 88 L 242 88 L 244 87 L 246 87 L 248 86 L 250 86 L 252 85 L 254 85 L 256 84 L 258 84 L 260 84 L 262 83 L 264 83 L 266 83 L 268 82 L 270 82 L 272 81 L 274 81 L 276 81 L 278 80 L 280 80 L 282 80 L 284 79 L 286 79 L 288 79 L 290 79 L 292 78 L 294 78 L 296 78 L 298 77 L 300 77 L 302 77 L 304 77 L 306 77 L 308 77 L 310 78 L 312 80 L 314 82 L 316 89 L 316 119 L 314 119 L 312 120 L 310 120 L 308 120 L 306 120 L 304 121 L 302 121 L 300 121 L 298 121 L 296 122 L 294 122 L 292 122 L 290 123 L 288 123 L 286 123 L 284 123 L 282 124 L 280 124 L 278 124 L 276 125 L 274 125 L 272 125 L 270 126 L 268 126 L 266 127 L 264 127 L 262 127 L 260 128 L 258 128 L 256 128 L 254 129 L 252 129 L 250 130 L 248 130 L 246 131 L 244 131 L 242 132 L 240 132 L 238 133 L 236 133 L 234 134 L 232 134 L 230 135 L 228 135 L 226 136 Z", b: [226, 77, 316, 136]},
        rb: {path: "M 810 163 L 814 159 L 815 154 L 814 148 L 810 144 L 804 143 L 800 141 L 796 139 L 792 138 L 788 136 L 784 135 L 780 134 L 776 133 L 772 131 L 768 130 L 764 129 L 760 128 L 756 127 L 752 126 L 748 125 L 744 124 L 740 124 L 736 123 L 732 122 L 728 121 L 724 121 L 720 120 L 716 119 L 712 119 L 708 118 L 704 118 L 700 117 L 696 117 L 692 116 L 688 116 L 684 115 L 680 115 L 676 114 L 672 114 L 668 113 L 664 113 L 656 114 L 652 118 L 651 124 L 652 129 L 656 133 L 664 135 L 668 135 L 672 136 L 676 136 L 680 137 L 684 137 L 688 138 L 692 138 L 696 139 L 700 139 L 704 140 L 708 140 L 712 141 L 716 141 L 720 142 L 724 143 L 728 143 L 732 144 L 736 145 L 740 146 L 744 146 L 748 147 L 752 148 L 756 149 L 760 150 L 764 151 L 768 152 L 772 153 L 776 155 L 780 156 L 784 157 L 788 158 L 792 160 L 796 161 L 800 163 L 804 165 Z", b: [651, 113, 815, 165]},
        rt: {path: "M 774 106 L 772 98 L 770 95 L 768 93 L 766 91 L 764 90 L 762 89 L 760 88 L 758 88 L 756 87 L 754 87 L 752 86 L 750 86 L 748 85 L 746 85 L 744 84 L 742 84 L 740 84 L 738 83 L 736 83 L 734 83 L 732 82 L 730 82 L 728 81 L 726 81 L 724 81 L 722 80 L 720 80 L 718 80 L 716 79 L 714 79 L 712 79 L 710 79 L 708 78 L 706 78 L 704 78 L 702 77 L 700 77 L 698 77 L 696 77 L 694 77 L 692 77 L 690 78 L 688 80 L 686 82 L 684 89 L 684 119 L 686 119 L 688 120 L 690 120 L 692 120 L 694 120 L 696 121 L 698 121 L 700 121 L 702 121 L 704 122 L 706 122 L 708 122 L 710 123 L 712 123 L 714 123 L 716 123 L 718 124 L 720 124 L 722 124 L 724 125 L 726 125 L 728 125 L 730 126 L 732 126 L 734 127 L 736 127 L 738 127 L 740 128 L 742 128 L 744 128 L 746 129 L 748 129 L 750 130 L 752 130 L 754 131 L 756 131 L 758 132 L 760 132 L 762 133 L 764 133 L 766 134 L 768 134 L 770 135 L 772 135 L 774 136 Z", b: [684, 77, 774, 136]}
    }
};

var DETAILS = {
    dualsense: [
        {kind: "shape", path: "M 324 118 L 676 118 L 652 282 C 650 289 645 291 638 291 L 362 291 C 355 291 350 289 348 282 Z", alpha: 0.06},
        {kind: "line", path: "M 320 121 L 350 284 C 352 292 358 294 366 294 L 634 294 C 642 294 648 292 650 284 L 680 121", alpha: 0.5, width: 3, glow: true},
        {kind: "dots", x: 500, y: 325, cols: 5, rows: 2, gap: 9, r: 2}
    ],
    xbox: [
        {kind: "dish", x: 385, y: 395, r: 66, alpha: 0.06, ring: true},
        {kind: "cross", x: 385, y: 395, l: 54, a: 34}
    ],
    generic: [
        {kind: "dish", x: 370, y: 400, r: 74, alpha: 0.05},
        {kind: "cross", x: 370, y: 400, l: 60, a: 36}
    ]
};

var LAYOUT = {
    dualsense: [
        {slot: "select", kind: "small", x: 293, y: 165, w: 14, h: 38, angle: -10},
        {slot: "start", kind: "small", x: 707, y: 165, w: 14, h: 38, angle: 10},
        {slot: "dpad_up", kind: "arm", cx: 228, cy: 258, l: 64, a: 38, dir: "up", split: true},
        {slot: "dpad_down", kind: "arm", cx: 228, cy: 258, l: 64, a: 38, dir: "down", split: true},
        {slot: "dpad_left", kind: "arm", cx: 228, cy: 258, l: 64, a: 38, dir: "left", split: true},
        {slot: "dpad_right", kind: "arm", cx: 228, cy: 258, l: 64, a: 38, dir: "right", split: true},
        {slot: "north", kind: "face", x: 775, y: 197, r: 28},
        {slot: "south", kind: "face", x: 775, y: 320, r: 28},
        {slot: "west", kind: "face", x: 713, y: 258, r: 28},
        {slot: "east", kind: "face", x: 837, y: 258, r: 28},
        {slot: "ls", kind: "stick", x: 357, y: 385, r: 52, axes: ["lx", "ly"]},
        {slot: "rs", kind: "stick", x: 643, y: 385, r: 52, axes: ["rx", "ry"]},
        {slot: "guide", kind: "face", x: 500, y: 383, r: 22, plain: true},
        {slot: "mute", kind: "small", x: 500, y: 432, w: 38, h: 11}
    ],
    xbox: [
        {slot: "guide", kind: "face", x: 500, y: 165, r: 35, plain: true},
        {slot: "select", kind: "small", x: 438, y: 253, w: 34, h: 34, round: true},
        {slot: "start", kind: "small", x: 562, y: 253, w: 34, h: 34, round: true},
        {slot: "share", kind: "small", x: 500, y: 300, w: 40, h: 23},
        {slot: "ls", kind: "stick", x: 270, y: 255, r: 54, axes: ["lx", "ly"]},
        {slot: "rs", kind: "stick", x: 612, y: 385, r: 54, axes: ["rx", "ry"]},
        {slot: "north", kind: "face", x: 726, y: 192, r: 29},
        {slot: "south", kind: "face", x: 726, y: 314, r: 29},
        {slot: "west", kind: "face", x: 665, y: 253, r: 29},
        {slot: "east", kind: "face", x: 787, y: 253, r: 29},
        {slot: "dpad_up", kind: "arm", cx: 385, cy: 395, l: 54, a: 34, dir: "up", split: false},
        {slot: "dpad_down", kind: "arm", cx: 385, cy: 395, l: 54, a: 34, dir: "down", split: false},
        {slot: "dpad_left", kind: "arm", cx: 385, cy: 395, l: 54, a: 34, dir: "left", split: false},
        {slot: "dpad_right", kind: "arm", cx: 385, cy: 395, l: 54, a: 34, dir: "right", split: false}
    ],
    generic: [
        {slot: "guide", kind: "face", x: 500, y: 170, r: 24, plain: true},
        {slot: "select", kind: "small", x: 430, y: 250, w: 36, h: 16},
        {slot: "start", kind: "small", x: 570, y: 250, w: 36, h: 16},
        {slot: "ls", kind: "stick", x: 250, y: 250, r: 52, axes: ["lx", "ly"]},
        {slot: "rs", kind: "stick", x: 630, y: 400, r: 52, axes: ["rx", "ry"]},
        {slot: "north", kind: "face", x: 750, y: 192, r: 28},
        {slot: "south", kind: "face", x: 750, y: 308, r: 28},
        {slot: "west", kind: "face", x: 692, y: 250, r: 28},
        {slot: "east", kind: "face", x: 808, y: 250, r: 28},
        {slot: "dpad_up", kind: "arm", cx: 370, cy: 400, l: 60, a: 36, dir: "up", split: false},
        {slot: "dpad_down", kind: "arm", cx: 370, cy: 400, l: 60, a: 36, dir: "down", split: false},
        {slot: "dpad_left", kind: "arm", cx: 370, cy: 400, l: 60, a: 36, dir: "left", split: false},
        {slot: "dpad_right", kind: "arm", cx: 370, cy: 400, l: 60, a: 36, dir: "right", split: false}
    ]
};

function shoulderSpecs(family) {
    var s = SHOULDERS[family];
    return [
        { slot: "lt", kind: "trigger", path: s.lt.path, b: s.lt.b, side: "l" },
        { slot: "rt", kind: "trigger", path: s.rt.path, b: s.rt.b, side: "r" },
        { slot: "lb", kind: "bumper", path: s.lb.path, b: s.lb.b, side: "l" },
        { slot: "rb", kind: "bumper", path: s.rb.path, b: s.rb.b, side: "r" }
    ];
}

function sonyButtons(kind) {
    var b = shoulderSpecs("dualsense").concat(LAYOUT.dualsense.filter(function (s) {
        return s.slot !== "mute" || kind === "dualsense" || kind === "dualsense-edge";
    }));
    if (kind === "dualsense-edge") {
        b.push({ slot: "fn_left", kind: "tab", x: 444, y: 432, w: 34, h: 15 });
        b.push({ slot: "fn_right", kind: "tab", x: 556, y: 432, w: 34, h: 15 });
        b.push({ slot: "paddle_left", kind: "paddle", x: 150, y: 500, w: 54, h: 96, ghost: true });
        b.push({ slot: "paddle_right", kind: "paddle", x: 850, y: 500, w: 54, h: 96, ghost: true });
    }
    return b;
}

// The Pro 3's second bumpers, inboard of L1/R1 along the generic body's top edge.
var PRO3_BUMPERS = [
    { slot: "paddle_l4", kind: "bumper", side: "l", b: [361, 108, 441, 133],
      path: "M 368 111 L 434 108 L 438 109 L 441 113 L 441 123 L 438 128 L 434 130 L 368 133 L 364 132 L 361 128 L 361 116 L 364 112 Z" },
    { slot: "paddle_r4", kind: "bumper", side: "r", b: [559, 108, 639, 133],
      path: "M 632 111 L 566 108 L 562 109 L 559 113 L 559 123 L 562 128 L 566 130 L 632 133 L 636 132 L 639 128 L 639 116 L 636 112 Z" }
];

function xboxButtons(kind) {
    var body = kind === "xbox" || kind === "xbox-elite" ? "xbox" : "generic";
    var b = shoulderSpecs(body).concat(LAYOUT[body].filter(function (s) {
        return s.slot !== "share" || kind === "xbox";
    }));
    if (kind === "xbox-elite") {
        b.push({ slot: "paddle_p3", kind: "paddle", x: 128, y: 520, w: 52, h: 96, ghost: true });
        b.push({ slot: "paddle_p4", kind: "paddle", x: 196, y: 548, w: 46, h: 70, ghost: true });
        b.push({ slot: "paddle_p1", kind: "paddle", x: 872, y: 520, w: 52, h: 96, ghost: true });
        b.push({ slot: "paddle_p2", kind: "paddle", x: 804, y: 548, w: 46, h: 70, ghost: true });
    }
    if (kind === "switch-pro")
        b.push({ slot: "capture", kind: "small", x: 430, y: 318, w: 28, h: 28 });
    if (kind === "8bitdo-pro-3") {
        b = b.concat(PRO3_BUMPERS);
        b.push({ slot: "star", kind: "small", x: 500, y: 300, w: 26, h: 26, round: true });
        b.push({ slot: "paddle_pl", kind: "paddle", x: 150, y: 500, w: 54, h: 96, ghost: true });
        b.push({ slot: "paddle_pr", kind: "paddle", x: 850, y: 500, w: 54, h: 96, ghost: true });
    }
    return b;
}

var SONY_KINDS = { "dualsense-edge": true, "dualsense": true, "dualshock4": true };
var DRAWN_KINDS = { "xbox-elite": true, "xbox": true, "switch-pro": true, "8bitdo-pro-3": true };

// A family without a body of its own is drawn on the generic one.
function of(family) {
    if (SONY_KINDS[family])
        return { view: VIEW, body: BODIES.dualsense, details: DETAILS.dualsense, buttons: sonyButtons(family) };
    var kind = DRAWN_KINDS[family] ? family : "generic";
    var body = kind === "xbox" || kind === "xbox-elite" ? "xbox" : "generic";
    return { view: VIEW, body: BODIES[body], details: DETAILS[body], buttons: xboxButtons(kind) };
}

// [x0, y0, x1, y1] of a body path: its points only, no curve control points beyond them.
function bounds(path) {
    var nums = path.split(/[^-\d.]+/).filter(function (t) { return t !== ""; }).map(Number);
    var x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (var i = 0; i + 1 < nums.length; i += 2) {
        x0 = Math.min(x0, nums[i]);
        x1 = Math.max(x1, nums[i]);
        y0 = Math.min(y0, nums[i + 1]);
        y1 = Math.max(y1, nums[i + 1]);
    }
    return [x0, y0, x1, y1];
}

function box(spec) {
    if (spec.kind === "face" || spec.kind === "stick")
        return { x: spec.x - spec.r, y: spec.y - spec.r, w: spec.r * 2, h: spec.r * 2 };
    if (spec.kind === "arm")
        return { x: spec.cx - spec.l, y: spec.cy - spec.l, w: spec.l * 2, h: spec.l * 2 };
    if (spec.kind === "paddle")
        return { x: spec.x - spec.w / 2, y: spec.y, w: spec.w, h: spec.h };
    if (spec.kind === "trigger" || spec.kind === "bumper")
        return { x: spec.b[0], y: spec.b[1], w: spec.b[2] - spec.b[0], h: spec.b[3] - spec.b[1] };
    var s = Math.max(spec.w, spec.h);
    return { x: spec.x - s / 2, y: spec.y - s / 2, w: s, h: s };
}
