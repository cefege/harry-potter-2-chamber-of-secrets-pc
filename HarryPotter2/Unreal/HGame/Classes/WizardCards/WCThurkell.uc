//===============================================================================
//  #87 Thaddeus Thurkell
//===============================================================================

class  WCThurkell extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardThurkellTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Thurkellsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardThurkellBigTexture FILE=TEXTURES\menu\Folio\Cards\Thurkellbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Thaddeus Thurkell";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=87
    skin(0)=Texture'HProps.skins.WizardCardThurkellTex0'
	textureBig=Texture'WizCardThurkellBigTexture'
	strDescriptionId="WizCard_0001"
}
